pub fn generate_mermaid_dsl(description: &str, diagram_type: &str) -> String {
    match diagram_type {
        "sequence" => format!("sequenceDiagram\n    A->>B: {description}\n    B-->>A: response"),
        "flowchart" | "flow" => format!("flowchart LR\n    A[Start] --> B[{description}] --> C[End]"),
        "class" => format!("classDiagram\n    class Entity\n    note \"{description}\""),
        _ => format!("graph LR\n    A[Start] --> B[{description}] --> C[End]"),
    }
}
