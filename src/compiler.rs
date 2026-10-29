use std::collections::HashSet;

use crate::ast::{Attribute, Node, Program, Value};

pub fn compile(program: &Program) -> String {
    compile_with_scope(program, "/")
}

pub fn compile_with_scope(program: &Program, scope: &str) -> String {
    let mut compiler = Compiler {
        output: String::new(),
        next_id: 0,
        states: HashSet::new(),
        scope: scope.to_owned(),
    };
    collect_states(&program.nodes, &mut compiler.states);
    compiler.output.push_str(
        "import init,{get_element,create_element,append,clear_children,set_text,set_class,\
         set_attribute,state,effect,listen,begin_render,navigate} from '/pkg/joeizor_ui.js';\n\
         await init();\n",
    );
    compiler.output.push_str(&format!(
        "begin_render({});\nconst root=get_element('app');clear_children(root);\n",
        js_string(scope)
    ));
    compiler.compile_nodes(&program.nodes, "root");
    compiler.output.push_str(
        "\nif(!globalThis.__joeizorRouterInitialized){globalThis.__joeizorRouterInitialized=true;\
         const reload=()=>import(`/__joeizor/app.js?route=${encodeURIComponent(location.pathname)}&v=${Date.now()}`);\
         const connect=()=>{const protocol=location.protocol==='https:'?'wss:':'ws:';\
         const socket=new WebSocket(`${protocol}//${location.host}/__joeizor/ws`);\
         socket.onmessage=reload;socket.onclose=()=>setTimeout(connect,500);};\
         window.addEventListener('popstate',reload);connect();}\n",
    );
    compiler.output
}

struct Compiler {
    output: String,
    next_id: usize,
    states: HashSet<String>,
    scope: String,
}

impl Compiler {
    fn compile_nodes(&mut self, nodes: &[Node], parent: &str) {
        for node in nodes {
            match node {
                Node::Text {
                    content,
                    attributes,
                } => {
                    let id = self.id();
                    self.output
                        .push_str(&format!("const n{id}=create_element('span');\n"));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output.push_str(&format!("append({parent},n{id});\n"));
                    let expression = interpolation(content, &self.states);
                    if contains_state(content, &self.states) {
                        self.output.push_str(&format!(
                            "effect(()=>set_text(n{id},String({expression}))); \n"
                        ));
                    } else {
                        self.output
                            .push_str(&format!("set_text(n{id},String({expression}));\n"));
                    }
                }
                Node::Button {
                    children,
                    attributes,
                } => {
                    let id = self.id();
                    self.output
                        .push_str(&format!("const n{id}=create_element('button');\n"));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output.push_str(&format!("append({parent},n{id});\n"));
                    self.compile_nodes(children, &format!("n{id}"));
                }
                Node::Section {
                    children,
                    attributes,
                } => {
                    let id = self.id();
                    self.output
                        .push_str(&format!("const n{id}=create_element('section');\n"));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output.push_str(&format!("append({parent},n{id});\n"));
                    self.compile_nodes(children, &format!("n{id}"));
                }
                Node::Navig {
                    to,
                    children,
                    attributes,
                } => {
                    let id = self.id();
                    self.output.push_str(&format!(
                        "const n{id}=create_element('a');set_attribute(n{id},'href',{});\
                         set_attribute(n{id},'data-jui-route',{});\n",
                        js_string(to),
                        js_string(to)
                    ));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output.push_str(&format!("append({parent},n{id});\n"));
                    self.output.push_str(&format!(
                        "listen(n{id},'click',event=>{{event.preventDefault();navigate({});}});\n",
                        js_string(to)
                    ));
                    if children.is_empty() {
                        self.output
                            .push_str(&format!("set_text(n{id},{});\n", js_string(to)));
                    } else {
                        self.compile_nodes(children, &format!("n{id}"));
                    }
                }
                Node::State { name, value } => {
                    self.output.push_str(&format!(
                        "const {}=state({},{},{});\n",
                        safe_identifier(name),
                        js_string(&self.scope),
                        js_string(name),
                        js_value(value)
                    ));
                }
                Node::Variable {
                    name,
                    value,
                    constant,
                } => {
                    let keyword = if *constant { "const" } else { "let" };
                    self.output.push_str(&format!(
                        "{keyword} {}={};\n",
                        safe_identifier(name),
                        js_value(value)
                    ));
                }
                Node::For {
                    item,
                    collection,
                    children,
                } => {
                    let collection = self.state_expression(collection);
                    self.output.push_str(&format!(
                        "for(const {} of ({} ?? [])){{\n",
                        safe_identifier(item),
                        collection
                    ));
                    self.compile_nodes(children, parent);
                    self.output.push_str("}\n");
                }
                Node::While {
                    condition,
                    children,
                } => {
                    self.output
                        .push_str(&format!("while({}){{\n", self.state_expression(condition)));
                    self.compile_nodes(children, parent);
                    self.output.push_str("}\n");
                }
                Node::DoWhile {
                    condition,
                    children,
                } => {
                    self.output.push_str("do{\n");
                    self.compile_nodes(children, parent);
                    self.output
                        .push_str(&format!("}}while({});\n", self.state_expression(condition)));
                }
                Node::If {
                    branches,
                    otherwise,
                } => {
                    let boundary = self.id();
                    self.output.push_str(&format!(
                        "const n{boundary}=create_element('span');\
                         set_class(n{boundary},'joeizor-fragment');append({parent},n{boundary});\n\
                         effect(()=>{{clear_children(n{boundary});\n"
                    ));
                    for (index, (condition, children)) in branches.iter().enumerate() {
                        let prefix = if index == 0 { "if" } else { "else if" };
                        self.output.push_str(&format!(
                            "{prefix}({}){{\n",
                            self.state_expression(condition)
                        ));
                        self.compile_nodes(children, &format!("n{boundary}"));
                        self.output.push_str("}\n");
                    }
                    if !otherwise.is_empty() {
                        self.output.push_str("else{\n");
                        self.compile_nodes(otherwise, &format!("n{boundary}"));
                        self.output.push_str("}\n");
                    }
                    self.output.push_str("});\n");
                }
                Node::Function {
                    name,
                    parameters,
                    children,
                } => {
                    let mut signature = parameters
                        .iter()
                        .map(|parameter| safe_identifier(parameter))
                        .collect::<Vec<_>>();
                    signature.push("parent=root".to_owned());
                    self.output.push_str(&format!(
                        "function {}({}){{\n",
                        safe_identifier(name),
                        signature.join(",")
                    ));
                    self.compile_nodes(children, "parent");
                    self.output.push_str("}\n");
                }
                Node::Call { name } => {
                    self.output
                        .push_str(&format!("{}({});\n", safe_identifier(name), parent));
                }
                Node::Class {
                    name,
                    base,
                    methods,
                } => {
                    let inheritance = base
                        .as_ref()
                        .map(|base| format!(" extends {}", safe_identifier(base)))
                        .unwrap_or_default();
                    self.output.push_str(&format!(
                        "class {}{}{{\n",
                        safe_identifier(name),
                        inheritance
                    ));
                    for method in methods {
                        if let Node::Function {
                            name,
                            parameters,
                            children,
                        } = method
                        {
                            let mut signature = parameters
                                .iter()
                                .map(|parameter| safe_identifier(parameter))
                                .collect::<Vec<_>>();
                            signature.push("parent=root".to_owned());
                            self.output.push_str(&format!(
                                "{}({}){{\n",
                                safe_identifier(name),
                                signature.join(",")
                            ));
                            self.compile_nodes(children, "parent");
                            self.output.push_str("}\n");
                        }
                    }
                    self.output.push_str("}\n");
                }
                Node::Instance { name, class } => {
                    self.output.push_str(&format!(
                        "const {}=new {}();\n",
                        safe_identifier(name),
                        safe_identifier(class)
                    ));
                }
                Node::MethodCall { receiver, method } => {
                    self.output.push_str(&format!(
                        "{}.{}({});\n",
                        safe_identifier(receiver),
                        safe_identifier(method),
                        parent
                    ));
                }
            }
        }
    }

    fn attributes(&mut self, target: &str, attributes: &[Attribute]) {
        for attribute in attributes {
            if attribute.name == "class" {
                self.output.push_str(&format!(
                    "set_class({target},{});\n",
                    js_string(&attribute.value)
                ));
            } else if let Some(event) = attribute.name.strip_prefix("on") {
                let event = event.to_ascii_lowercase();
                if let Some(state_name) = attribute
                    .value
                    .strip_suffix("++")
                    .or_else(|| attribute.value.strip_suffix("--"))
                    .filter(|name| self.states.contains(*name))
                {
                    let delta = if attribute.value.ends_with("++") {
                        "+1"
                    } else {
                        "-1"
                    };
                    self.output.push_str(&format!(
                        "listen({target},{},()=>{{{}.set({}.get(){delta});}});\n",
                        js_string(&event),
                        safe_identifier(state_name),
                        safe_identifier(state_name)
                    ));
                }
            } else {
                self.output.push_str(&format!(
                    "set_attribute({target},{},{});\n",
                    js_string(&attribute.name),
                    js_string(&attribute.value)
                ));
            }
        }
    }

    fn state_expression(&self, expression: &str) -> String {
        let expression = expression.trim();
        if self.states.contains(expression) {
            format!("{}.get()", safe_identifier(expression))
        } else {
            safe_identifier(expression)
        }
    }

    fn id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn collect_states(nodes: &[Node], states: &mut HashSet<String>) {
    for node in nodes {
        match node {
            Node::State { name, .. } => {
                states.insert(name.clone());
            }
            Node::Section { children, .. }
            | Node::Button { children, .. }
            | Node::For { children, .. }
            | Node::While { children, .. }
            | Node::DoWhile { children, .. } => collect_states(children, states),
            Node::Navig { children, .. } | Node::Function { children, .. } => {
                collect_states(children, states)
            }
            Node::If {
                branches,
                otherwise,
            } => {
                for (_, children) in branches {
                    collect_states(children, states);
                }
                collect_states(otherwise, states);
            }
            Node::Class { methods, .. } => collect_states(methods, states),
            _ => {}
        }
    }
}

fn contains_state(value: &str, states: &HashSet<String>) -> bool {
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open + 1..].find('}') else {
            return false;
        };
        if states.contains(rest[open + 1..open + 1 + close].trim()) {
            return true;
        }
        rest = &rest[open + close + 2..];
    }
    false
}

fn js_value(value: &Value) -> String {
    match value {
        Value::String(value) => js_string(value),
        Value::Number(value) | Value::Boolean(value) => value.clone(),
        Value::Identifier(value) => safe_identifier(value),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(js_value).collect::<Vec<_>>().join(",")
        ),
    }
}

fn js_string(value: &str) -> String {
    format!("{value:?}")
}

fn safe_identifier(value: &str) -> String {
    if value.chars().enumerate().all(|(index, ch)| {
        ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
    }) {
        value.to_owned()
    } else {
        format!("globalThis[{}]", js_string(value))
    }
}

fn interpolation(value: &str, states: &HashSet<String>) -> String {
    let mut parts = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let before = &rest[..open];
        if let Some(close) = rest[open + 1..].find('}') {
            let expression = rest[open + 1..open + 1 + close].trim();
            parts.push(js_string(before));
            let expression = compile_expression(expression, states);
            parts.push(format!("String({expression})"));
            rest = &rest[open + close + 2..];
        } else {
            break;
        }
    }
    if parts.is_empty() {
        return js_string(value);
    }
    parts.push(js_string(rest));
    format!("({})", parts.join("+"))
}

fn compile_expression(expression: &str, states: &HashSet<String>) -> String {
    let mut parts = expression.split('.');
    let Some(first) = parts.next() else {
        return "undefined".to_owned();
    };
    let mut compiled = if states.contains(first) {
        format!("{}.get()", safe_identifier(first))
    } else {
        safe_identifier(first)
    };
    for part in parts {
        if part.chars().enumerate().all(|(index, ch)| {
            ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
        }) {
            compiled.push('.');
            compiled.push_str(part);
        } else {
            return "undefined".to_owned();
        }
    }
    compiled
}

#[cfg(test)]
mod tests {
    use super::{compile, compile_with_scope};
    use crate::parser;

    #[test]
    fn components_render_into_their_call_site() {
        let program = parser::parse(
            r#"
                func Greeting()
                    text "Hola" end
                end
                sec class="container"
                    Greeting()
                end
            "#,
        )
        .expect("el componente debe ser valido");

        let output = compile(&program);
        assert!(output.contains("function Greeting(parent=root)"));
        assert!(output.contains("Greeting(n1);"));
    }

    #[test]
    fn state_text_and_button_updates_use_wasm_signals() {
        let program = parser::parse(
            r#"
                state count = 0
                text "Contador: {count}" end
                btn class="btn btn-primary" onClick={count++}
                    text "Sumar" end
                end
            "#,
        )
        .expect("el estado y el evento deben ser validos");

        let output = compile_with_scope(&program, "/home");
        assert!(output.contains("count=state(\"/home\",\"count\",0)"));
        assert!(output.contains("effect(()=>set_text"));
        assert!(output.contains("count.set(count.get()+1)"));
        assert!(output.contains("begin_render(\"/home\")"));
        assert!(output.contains("window.addEventListener('popstate',reload)"));
    }

    #[test]
    fn state_conditions_rerender_only_their_fragment() {
        let program = parser::parse(
            r#"
                state visible = true
                if visible
                    text "Visible" end
                else
                    text "Hidden" end
                end
            "#,
        )
        .expect("el condicional reactivo debe ser valido");

        let output = compile_with_scope(&program, "/conditional");
        assert!(output.contains("effect(()=>{clear_children(n0)"));
        assert!(output.contains("if(visible.get())"));
        assert!(output.contains("visible=state(\"/conditional\",\"visible\",true)"));
    }
}
