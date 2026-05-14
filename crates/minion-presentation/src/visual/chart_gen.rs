pub fn generate_chart_spec(data_description: &str, chart_type: &str) -> serde_json::Value {
    serde_json::json!({
        "type": chart_type,
        "title": data_description,
        "data": { "labels": ["A","B","C"], "datasets": [{"label": data_description, "data": [0,0,0]}] },
        "options": { "responsive": true }
    })
}
