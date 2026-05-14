pub fn template_for(spec_hint: &str) -> String {
    match spec_hint {
        "arrow" => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60">
  <defs><marker id="ah" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto"><polygon points="0 0,10 3.5,0 7" fill="#6366f1"/></marker></defs>
  <line x1="10" y1="30" x2="180" y2="30" stroke="#6366f1" stroke-width="3" marker-end="url(#ah)"/>
</svg>"##.into(),
        "process" => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 80">
  <rect x="0" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <rect x="150" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <rect x="300" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <line x1="100" y1="40" x2="150" y2="40" stroke="#fff" stroke-width="2"/>
  <line x1="250" y1="40" x2="300" y2="40" stroke="#fff" stroke-width="2"/>
</svg>"##.into(),
        "kpi" => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
  <rect width="200" height="100" rx="12" fill="#1c1c24"/>
  <text x="100" y="55" text-anchor="middle" font-size="42" font-weight="700" fill="#6366f1">0</text>
  <text x="100" y="80" text-anchor="middle" font-size="14" fill="#a0a0b4">Label</text>
</svg>"##.into(),
        "comparison" => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 120">
  <rect x="0" y="0" width="140" height="120" rx="8" fill="#1c1c24"/>
  <rect x="160" y="0" width="140" height="120" rx="8" fill="#1c1c24"/>
  <text x="70" y="65" text-anchor="middle" font-size="16" fill="#fff">A</text>
  <text x="230" y="65" text-anchor="middle" font-size="16" fill="#fff">B</text>
</svg>"##.into(),
        _ => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 100">
  <rect width="300" height="100" rx="8" fill="#1c1c24" stroke="#6366f1" stroke-width="1"/>
  <text x="150" y="55" text-anchor="middle" font-size="16" fill="#a0a0b4">Visual</text>
</svg>"##.into(),
    }
}
