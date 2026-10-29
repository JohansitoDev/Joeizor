use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use std::collections::HashSet;

use crate::ast::{Attribute, Node, Program, Value};

#[derive(Parser)]
#[grammar = "jui.pest"]
struct JuiParser;

type PairResult<T> = Result<T, String>;

pub fn parse(source: &str) -> PairResult<Program> {
    let pairs = JuiParser::parse(Rule::program, source).map_err(|error| error.to_string())?;
    let mut program = Program::default();
    for pair in pairs {
        if pair.as_rule() == Rule::program {
            for statement in pair.into_inner() {
                if statement.as_rule() == Rule::statement {
                    if let Some(inner) = statement.into_inner().next() {
                        program.nodes.push(parse_node(inner)?);
                    }
                }
            }
        }
    }
    validate_events(&program.nodes)?;
    Ok(program)
}

fn validate_events(nodes: &[Node]) -> PairResult<()> {
    let states = collect_states(nodes);
    validate_events_in(nodes, &states)
}

fn validate_events_in(nodes: &[Node], states: &HashSet<String>) -> PairResult<()> {
    for node in nodes {
        let attributes = match node {
            Node::Text { attributes, .. }
            | Node::Button { attributes, .. }
            | Node::Section { attributes, .. }
            | Node::Navig { attributes, .. } => attributes.as_slice(),
            _ => &[],
        };
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.name.starts_with("on"))
        {
            if attribute.name != "onClick" {
                return Err(format!(
                    "evento no soportado: {}; usa onClick",
                    attribute.name
                ));
            }
            let state = attribute
                .value
                .strip_suffix("++")
                .or_else(|| attribute.value.strip_suffix("--"))
                .ok_or("onClick requiere una accion como {count++}")?;
            if !states.contains(state) {
                return Err(format!("onClick modifica el estado no declarado: {state}"));
            }
        }
        match node {
            Node::Section { children, .. }
            | Node::Button { children, .. }
            | Node::For { children, .. }
            | Node::While { children, .. }
            | Node::DoWhile { children, .. }
            | Node::Function { children, .. }
            | Node::Navig { children, .. } => validate_events_in(children, states)?,
            Node::If {
                branches,
                otherwise,
            } => {
                for (_, children) in branches {
                    validate_events_in(children, states)?;
                }
                validate_events_in(otherwise, states)?;
            }
            Node::Class { methods, .. } => validate_events_in(methods, states)?,
            _ => {}
        }
    }
    Ok(())
}

fn collect_states(nodes: &[Node]) -> HashSet<String> {
    let mut states = HashSet::new();
    for node in nodes {
        match node {
            Node::State { name, .. } => {
                states.insert(name.clone());
            }
            Node::Section { children, .. }
            | Node::Button { children, .. }
            | Node::For { children, .. }
            | Node::While { children, .. }
            | Node::DoWhile { children, .. }
            | Node::Navig { children, .. }
            | Node::Function { children, .. } => states.extend(collect_states(children)),
            Node::If {
                branches,
                otherwise,
            } => {
                for (_, children) in branches {
                    states.extend(collect_states(children));
                }
                states.extend(collect_states(otherwise));
            }
            Node::Class { methods, .. } => states.extend(collect_states(methods)),
            _ => {}
        }
    }
    states
}

fn parse_node(pair: Pair<'_, Rule>) -> PairResult<Node> {
    match pair.as_rule() {
        Rule::class_decl => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .ok_or("class requiere un nombre")?
                .as_str()
                .to_owned();
            let mut base = None;
            let mut methods = Vec::new();
            for part in inner {
                match part.as_rule() {
                    Rule::identifier if base.is_none() => base = Some(part.as_str().to_owned()),
                    Rule::statement => {
                        let method = parse_statement(part)?;
                        if !matches!(&method, Node::Function { .. }) {
                            return Err("las clases solo admiten metodos func por ahora".to_owned());
                        }
                        methods.push(method);
                    }
                    _ => {}
                }
            }
            Ok(Node::Class {
                name,
                base,
                methods,
            })
        }
        Rule::instance_decl => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .ok_or("instancia sin nombre")?
                .as_str()
                .to_owned();
            let class = inner
                .next()
                .ok_or("new requiere una clase")?
                .as_str()
                .to_owned();
            Ok(Node::Instance { name, class })
        }
        Rule::method_call => {
            let mut inner = pair.into_inner();
            let receiver = inner
                .next()
                .ok_or("llamada sin objeto")?
                .as_str()
                .to_owned();
            let method = inner
                .next()
                .ok_or("llamada sin metodo")?
                .as_str()
                .to_owned();
            Ok(Node::MethodCall { receiver, method })
        }
        Rule::function_decl => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .ok_or("func requiere un nombre")?
                .as_str()
                .to_owned();
            let mut parameters = Vec::new();
            let mut children = Vec::new();
            for part in inner {
                match part.as_rule() {
                    Rule::parameter_list => {
                        parameters.extend(part.into_inner().map(|p| p.as_str().to_owned()));
                    }
                    Rule::statement => children.push(parse_statement(part)?),
                    _ => {}
                }
            }
            Ok(Node::Function {
                name,
                parameters,
                children,
            })
        }
        Rule::function_call => {
            let name = pair
                .into_inner()
                .next()
                .ok_or("llamada sin nombre")?
                .as_str()
                .to_owned();
            Ok(Node::Call { name })
        }
        Rule::state_decl | Rule::constant_decl | Rule::variable_decl => {
            let constant = pair.as_rule() == Rule::constant_decl;
            let state = pair.as_rule() == Rule::state_decl;
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .ok_or("declaracion sin nombre")?
                .as_str()
                .to_owned();
            let value = parse_value(inner.next().ok_or("declaracion sin valor")?)?;
            if state {
                Ok(Node::State { name, value })
            } else {
                Ok(Node::Variable {
                    name,
                    value,
                    constant,
                })
            }
        }
        Rule::if_stmt => parse_if(pair),
        Rule::for_stmt => {
            let mut inner = pair.into_inner();
            let item = inner
                .next()
                .ok_or("for requiere variable")?
                .as_str()
                .to_owned();
            let collection = inner
                .next()
                .ok_or("for requiere coleccion")?
                .as_str()
                .to_owned();
            let children = inner.map(parse_statement).collect::<PairResult<Vec<_>>>()?;
            Ok(Node::For {
                item,
                collection,
                children,
            })
        }
        Rule::while_stmt | Rule::do_while_stmt => {
            let is_do_while = pair.as_rule() == Rule::do_while_stmt;
            let mut condition = None;
            let mut children = Vec::new();
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::condition => condition = Some(part.as_str().to_owned()),
                    Rule::statement => children.push(parse_statement(part)?),
                    _ => {}
                }
            }
            let condition = condition.ok_or("bucle requiere una condicion")?;
            if is_do_while {
                Ok(Node::DoWhile {
                    condition,
                    children,
                })
            } else {
                Ok(Node::While {
                    condition,
                    children,
                })
            }
        }
        Rule::text_element => {
            let (attributes, content) = parse_element_parts(pair)?;
            Ok(Node::Text {
                content,
                attributes,
            })
        }
        Rule::button_element => {
            let mut attributes = Vec::new();
            let mut children = Vec::new();
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::attribute => attributes.push(parse_attribute(part)?),
                    Rule::statement => children.push(parse_statement(part)?),
                    _ => {}
                }
            }
            Ok(Node::Button {
                children,
                attributes,
            })
        }
        Rule::section_element => {
            let mut attributes = Vec::new();
            let mut children = Vec::new();
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::attribute => attributes.push(parse_attribute(part)?),
                    Rule::statement => children.push(parse_statement(part)?),
                    _ => {}
                }
            }
            Ok(Node::Section {
                children,
                attributes,
            })
        }
        Rule::navig_element => {
            let mut inner = pair.into_inner();
            let to = unquote(inner.next().ok_or("navig requiere una ruta")?.as_str());
            let mut attributes = Vec::new();
            let mut children = Vec::new();
            for part in inner {
                match part.as_rule() {
                    Rule::attribute => attributes.push(parse_attribute(part)?),
                    Rule::statement => children.push(parse_statement(part)?),
                    _ => {}
                }
            }
            Ok(Node::Navig {
                to,
                children,
                attributes,
            })
        }
        Rule::navig_short => {
            let to = pair.into_inner().next().ok_or("navig requiere una ruta")?;
            Ok(Node::Navig {
                to: unquote(to.as_str()),
                children: Vec::new(),
                attributes: Vec::new(),
            })
        }
        other => Err(format!("instruccion no soportada: {other:?}")),
    }
}

fn parse_if(pair: Pair<'_, Rule>) -> PairResult<Node> {
    let mut inner = pair.into_inner();
    let mut branches = Vec::new();
    let mut otherwise = Vec::new();
    let first_condition = inner
        .next()
        .ok_or("if requiere una condicion")?
        .as_str()
        .to_owned();
    let mut first_children = Vec::new();
    for part in inner {
        match part.as_rule() {
            Rule::statement => first_children.push(parse_statement(part)?),
            Rule::else_if_clause => {
                let mut clause = part.into_inner();
                let condition = clause
                    .next()
                    .ok_or("else if requiere una condicion")?
                    .as_str()
                    .to_owned();
                let children = clause
                    .map(parse_statement)
                    .collect::<PairResult<Vec<_>>>()?;
                branches.push((condition, children));
            }
            Rule::else_clause => {
                otherwise = part
                    .into_inner()
                    .map(parse_statement)
                    .collect::<PairResult<Vec<_>>>()?;
            }
            _ => {}
        }
    }
    branches.insert(0, (first_condition, first_children));
    Ok(Node::If {
        branches,
        otherwise,
    })
}

fn parse_element_parts(pair: Pair<'_, Rule>) -> PairResult<(Vec<Attribute>, String)> {
    let mut attributes = Vec::new();
    let mut content = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::attribute => attributes.push(parse_attribute(part)?),
            Rule::string => content = Some(unquote(part.as_str())),
            _ => {}
        }
    }
    Ok((attributes, content.ok_or("text requiere contenido")?))
}

fn parse_attribute(pair: Pair<'_, Rule>) -> PairResult<Attribute> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .ok_or("atributo sin nombre")?
        .as_str()
        .to_owned();
    let value_pair = inner.next().ok_or("atributo sin valor")?;
    let value = if value_pair.as_rule() == Rule::string {
        unquote(value_pair.as_str())
    } else {
        let expression = value_pair.as_str();
        expression[1..expression.len() - 1].to_owned()
    };
    Ok(Attribute { name, value })
}

fn parse_value(pair: Pair<'_, Rule>) -> PairResult<Value> {
    let inner = pair.into_inner().next().ok_or("valor vacio")?;
    Ok(match inner.as_rule() {
        Rule::string => Value::String(unquote(inner.as_str())),
        Rule::number => Value::Number(inner.as_str().to_owned()),
        Rule::boolean => Value::Boolean(inner.as_str().to_owned()),
        Rule::identifier => Value::Identifier(inner.as_str().to_owned()),
        Rule::array => Value::Array(
            inner
                .into_inner()
                .map(parse_value)
                .collect::<PairResult<Vec<_>>>()?,
        ),
        _ => return Err(format!("valor no soportado: {}", inner.as_str())),
    })
}

fn parse_statement(pair: Pair<'_, Rule>) -> PairResult<Node> {
    let inner = pair.into_inner().next().ok_or("sentencia vacia")?;
    parse_node(inner)
}

fn unquote(value: &str) -> String {
    value[1..value.len() - 1]
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::ast::Node;

    #[test]
    fn parses_functions_control_flow_and_nested_elements() {
        let program = parse(
            r#"
                state count = 0
                name = "Johan"
                PI = 3.1416
                users = ["Ana", "Luis"]
                func Home()
                    sec class="p-4"
                        for user in users
                            text "{user}" end
                        end
                        if count
                            text "uno" end
                        else if name
                            text "dos" end
                        else
                            text "tres" end
                        end
                    end
                end
                Home()
            "#,
        )
        .expect("el ejemplo debe ser valido");

        assert_eq!(program.nodes.len(), 6);
        assert!(matches!(&program.nodes[4], Node::Function { .. }));
        assert!(matches!(&program.nodes[5], Node::Call { .. }));
    }

    #[test]
    fn parses_while_do_while_and_inherited_classes() {
        let program = parse(
            r#"
                ready = false
                while ready
                    text "wait" end
                end
                do
                    text "once" end
                end while ready
                class Animal
                    func speak()
                        text "animal" end
                    end
                end
                class Cat extends Animal
                    func speak()
                        text "cat" end
                    end
                end
                cat = new Cat()
                cat.speak()
            "#,
        )
        .expect("los bucles y las clases deben ser validos");

        assert!(matches!(&program.nodes[1], Node::While { .. }));
        assert!(matches!(&program.nodes[2], Node::DoWhile { .. }));
        assert!(matches!(
            &program.nodes[4],
            Node::Class {
                base: Some(base),
                ..
            } if base == "Animal"
        ));
        assert!(matches!(&program.nodes[5], Node::Instance { .. }));
        assert!(matches!(&program.nodes[6], Node::MethodCall { .. }));
    }

    #[test]
    fn rejects_click_handlers_that_cannot_update_a_declared_state() {
        let error = parse(
            r#"
                btn onClick={missing++}
                    text "Bad handler" end
                end
            "#,
        )
        .expect_err("no debe aceptar referencias a estado inexistente");
        assert!(error.contains("missing"));
    }
}
