use crate::ast::{Attribute, Node, Program, Value};

pub fn compile(program: &Program) -> String {
    let mut compiler = Compiler {
        output: String::new(),
        next_id: 0,
    };
    compiler.output.push_str(
        "const root=document.getElementById('app');\n\
         if(!root) throw new Error('Joeizor UI: falta el nodo #app');\n\
         root.replaceChildren();\n",
    );
    compiler.compile_nodes(&program.nodes, "root");
    compiler.output.push_str(
        "\n// Navegacion e recarga del modulo sin refrescar el documento.\n\
         if(!window.__joeizorRouterInitialized){window.__joeizorRouterInitialized=true;\
         const loadRoute=()=>import(`/__joeizor/app.js?route=${encodeURIComponent(location.pathname)}&v=${Date.now()}`);\
         document.addEventListener('click',e=>{const a=e.target.closest('a[data-jui-route]');\
         if(a){e.preventDefault();history.pushState({},'',a.dataset.juiRoute);loadRoute();}});\
         window.addEventListener('popstate',loadRoute);\
         const changes=new EventSource('/__joeizor/events');\
         changes.onmessage=()=>loadRoute();}\n",
    );
    compiler.output
}

struct Compiler {
    output: String,
    next_id: usize,
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
                    self.output.push_str(&format!(
                        "const n{id}=document.createElement('span');n{id}.textContent={};\n",
                        interpolation(content)
                    ));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output
                        .push_str(&format!("{parent}.appendChild(n{id});\n"));
                }
                Node::Section {
                    children,
                    attributes,
                } => {
                    let id = self.id();
                    self.output
                        .push_str(&format!("const n{id}=document.createElement('section');\n"));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output
                        .push_str(&format!("{parent}.appendChild(n{id});\n"));
                    self.compile_nodes(children, &format!("n{id}"));
                }
                Node::Navig {
                    to,
                    children,
                    attributes,
                } => {
                    let id = self.id();
                    self.output.push_str(&format!(
                        "const n{id}=document.createElement('a');n{id}.href={};\
                         n{id}.dataset.juiRoute={};\n",
                        js_string(to),
                        js_string(to)
                    ));
                    self.attributes(&format!("n{id}"), attributes);
                    self.output
                        .push_str(&format!("{parent}.appendChild(n{id});\n"));
                    if children.is_empty() {
                        self.output
                            .push_str(&format!("n{id}.textContent={};\n", js_string(to)));
                    } else {
                        self.compile_nodes(children, &format!("n{id}"));
                    }
                }
                Node::State { name, value } => {
                    self.output.push_str(&format!(
                        "let {}={};\n",
                        safe_identifier(name),
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
                    self.output.push_str(&format!(
                        "for(const {} of ({} ?? [])){{\n",
                        safe_identifier(item),
                        safe_identifier(collection)
                    ));
                    self.compile_nodes(children, parent);
                    self.output.push_str("}\n");
                }
                Node::While {
                    condition,
                    children,
                } => {
                    self.output
                        .push_str(&format!("while({}){{\n", safe_identifier(condition)));
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
                        .push_str(&format!("}}while({});\n", safe_identifier(condition)));
                }
                Node::If {
                    branches,
                    otherwise,
                } => {
                    for (index, (condition, children)) in branches.iter().enumerate() {
                        let prefix = if index == 0 { "if" } else { "else if" };
                        self.output
                            .push_str(&format!("{prefix}({}){{\n", safe_identifier(condition)));
                        self.compile_nodes(children, parent);
                        self.output.push_str("}\n");
                    }
                    if !otherwise.is_empty() {
                        self.output.push_str("else{\n");
                        self.compile_nodes(otherwise, parent);
                        self.output.push_str("}\n");
                    }
                }
                Node::Function {
                    name,
                    parameters,
                    children,
                } => {
                    self.output.push_str(&format!(
                        "function {}({}){{\n",
                        safe_identifier(name),
                        parameters
                            .iter()
                            .map(|p| safe_identifier(p))
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                    self.compile_nodes(children, parent);
                    self.output.push_str("}\n");
                }
                Node::Call { name } => {
                    self.output
                        .push_str(&format!("{}();\n", safe_identifier(name)));
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
                            self.output.push_str(&format!(
                                "{}({}){{\n",
                                safe_identifier(name),
                                parameters
                                    .iter()
                                    .map(|parameter| safe_identifier(parameter))
                                    .collect::<Vec<_>>()
                                    .join(",")
                            ));
                            self.compile_nodes(children, parent);
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
                        "{}.{}();\n",
                        safe_identifier(receiver),
                        safe_identifier(method)
                    ));
                }
            }
        }
    }

    fn attributes(&mut self, target: &str, attributes: &[Attribute]) {
        for attribute in attributes {
            match attribute.name.as_str() {
                "class" => self.output.push_str(&format!(
                    "{target}.className={};\n",
                    js_string(&attribute.value)
                )),
                "id" | "title" | "role" => self.output.push_str(&format!(
                    "{target}.setAttribute({},{});\n",
                    js_string(&attribute.name),
                    js_string(&attribute.value)
                )),
                _ => {}
            }
        }
    }

    fn id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
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
    format!("{:?}", value)
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

fn interpolation(value: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let before = &rest[..open];
        if let Some(close) = rest[open + 1..].find('}') {
            let expression = rest[open + 1..open + 1 + close].trim();
            parts.push(js_string(before));
            parts.push(format!("String({})", expression));
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
