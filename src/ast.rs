#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(String),
    Boolean(String),
    Identifier(String),
    Array(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum Node {
    Text {
        content: String,
        attributes: Vec<Attribute>,
    },
    Button {
        children: Vec<Node>,
        attributes: Vec<Attribute>,
    },
    Section {
        children: Vec<Node>,
        attributes: Vec<Attribute>,
    },
    Navig {
        to: String,
        children: Vec<Node>,
        attributes: Vec<Attribute>,
    },
    State {
        name: String,
        value: Value,
    },
    Variable {
        name: String,
        value: Value,
        constant: bool,
    },
    For {
        item: String,
        collection: String,
        children: Vec<Node>,
    },
    While {
        condition: String,
        children: Vec<Node>,
    },
    DoWhile {
        condition: String,
        children: Vec<Node>,
    },
    If {
        branches: Vec<(String, Vec<Node>)>,
        otherwise: Vec<Node>,
    },
    Function {
        name: String,
        parameters: Vec<String>,
        children: Vec<Node>,
    },
    Call {
        name: String,
    },
    Class {
        name: String,
        base: Option<String>,
        methods: Vec<Node>,
    },
    Instance {
        name: String,
        class: String,
    },
    MethodCall {
        receiver: String,
        method: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub nodes: Vec<Node>,
}
