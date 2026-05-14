use roxmltree::Document;
use std::fmt::Write;

const ALLOWED: &[&str] = &[
    "svg",
    "g",
    "defs",
    "symbol",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "title",
    "desc",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "mask",
    "filter",
    "feGaussianBlur",
    "feColorMatrix",
    "feComposite",
    "feBlend",
    "animate",
    "animateTransform",
    "use",
    "pattern",
];

static HREF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
fn href_re() -> &'static regex::Regex {
    HREF_RE.get_or_init(|| regex::Regex::new(r"^#[a-zA-Z0-9_-]+$").unwrap())
}

pub fn sanitize_svg(input: &str) -> Result<String, String> {
    let doc = Document::parse(input).map_err(|e| e.to_string())?;
    let mut out = String::with_capacity(input.len());
    serialize_node(doc.root(), &mut out)?;
    Ok(out)
}

fn serialize_node(node: roxmltree::Node, out: &mut String) -> Result<(), String> {
    match node.node_type() {
        roxmltree::NodeType::Root => {
            for child in node.children() {
                serialize_node(child, out)?;
            }
        }
        roxmltree::NodeType::Element => {
            let tag = node.tag_name().name();
            if !ALLOWED.contains(&tag) {
                return Ok(());
            }
            write!(out, "<{tag}").unwrap();
            for attr in node.attributes() {
                let (name, value) = (attr.name(), attr.value());
                if name.starts_with("on") {
                    continue;
                }
                if (name == "href" || name == "xlink:href") && tag == "use" && !href_re().is_match(value) {
                    return Err(format!("invalid use href: {value}"));
                }
                if tag == "feGaussianBlur" && name == "stdDeviation" {
                    let v: f64 = value.parse().unwrap_or(0.0);
                    write!(out, " stdDeviation=\"{}\"", v.min(20.0)).unwrap();
                    continue;
                }
                write!(out, " {name}=\"{value}\"").unwrap();
            }
            if node.has_children() {
                write!(out, ">").unwrap();
                for child in node.children() {
                    serialize_node(child, out)?;
                }
                write!(out, "</{tag}>").unwrap();
            } else {
                write!(out, "/>").unwrap();
            }
        }
        roxmltree::NodeType::Text => {
            out.push_str(node.text().unwrap_or(""));
        }
        _ => {}
    }
    Ok(())
}
