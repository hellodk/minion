# Presentation — Plan A: Bug Fixes + Interactive HTML

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the play_order desync bug, resolve clippy warnings, and upgrade the Interactive HTML export to a proper click-through presentation viewer.

**Architecture:** play_order fix is in orchestrator.rs + bundle.rs. Clippy is inline fixes. HTML export replaces the static render in ExportDialog.tsx with a full self-contained viewer.

**Tech Stack:** Rust (orchestrator, bundle, validate), TypeScript (ExportDialog.tsx).

---

## Files

| Action  | Path |
|---------|------|
| Modify  | `crates/minion-presentation/src/bundle.rs` |
| Modify  | `crates/minion-presentation/src/orchestrator.rs` |
| Test    | `crates/minion-presentation/tests/bundle_tests.rs` |
| Modify  | `crates/minion-presentation/src/agents/visual.rs` |
| Modify  | `crates/minion-presentation/src/visual/svg_sanitizer.rs` |
| Modify  | `ui/src/pages/presentation/ExportDialog.tsx` |

---

## Task 1: play_order bug fix

**Context:** `bundle::apply_patch` for `DeleteSlide` already calls `deck.play_order.retain(|id| id != &slide_id)` — so a single direct delete is fine. The real gap is that `DesignCriticAgent` patches are applied in a loop in `orchestrator.rs` with no integrity check afterward. If a future patch somehow corrupts the ordering (duplicate, orphan), it will silently persist. The fix is a `validate_and_repair_play_order` helper in `bundle.rs` that rebuilds from scratch when validation fails, and a call to it in the orchestrator after all critic patches are applied.

**Files:**
- Modify: `crates/minion-presentation/src/bundle.rs`
- Modify: `crates/minion-presentation/src/orchestrator.rs`
- Test: `crates/minion-presentation/tests/bundle_tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/minion-presentation/tests/bundle_tests.rs` after the existing `apply_patch_set_meta_updates_title` test:

```rust
#[test]
fn delete_slide_removes_from_play_order() {
    use minion_presentation::schema::types::{SectionId, LayoutKind};

    let mut deck = minimal_deck("Order Test");
    let sec_id = SectionId::new();
    deck.sections.push(minion_presentation::schema::types::Section {
        id: sec_id.clone(),
        title: "S1".into(),
        slides: vec![
            Slide::new(sec_id.clone(), 0.0, 0.0, LayoutKind::Title),
            Slide::new(sec_id.clone(), 0.0, 0.0, LayoutKind::Blank),
        ],
    });
    let id_a = deck.sections[0].slides[0].id.clone();
    let id_b = deck.sections[0].slides[1].id.clone();
    deck.play_order = vec![id_a.clone(), id_b.clone()];

    apply_patch(&mut deck, DeckPatch::DeleteSlide { slide_id: id_a.clone() });

    assert!(!deck.play_order.contains(&id_a), "deleted slide must not remain in play_order");
    assert_eq!(deck.play_order, vec![id_b], "play_order should contain only the surviving slide");
}

#[test]
fn validate_and_repair_play_order_fixes_orphan() {
    use minion_presentation::schema::types::{SectionId, LayoutKind};
    use minion_presentation::bundle::validate_and_repair_play_order;

    let mut deck = minimal_deck("Repair Test");
    let sec_id = SectionId::new();
    deck.sections.push(minion_presentation::schema::types::Section {
        id: sec_id.clone(),
        title: "S1".into(),
        slides: vec![Slide::new(sec_id.clone(), 0.0, 0.0, LayoutKind::Title)],
    });
    let real_id = deck.sections[0].slides[0].id.clone();
    let ghost_id = SlideId::new(); // not in sections

    // Corrupt play_order: contains a ghost and is missing the real slide
    deck.play_order = vec![ghost_id];

    validate_and_repair_play_order(&mut deck);

    assert_eq!(deck.play_order, vec![real_id], "repair must rebuild from all_slides()");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p minion-presentation bundle 2>&1 | tail -20
```

Expected: two failures — `delete_slide_removes_from_play_order` (compile error: `validate_and_repair_play_order` not found) and `validate_and_repair_play_order_fixes_orphan` (same).

- [ ] **Step 3: Add `validate_and_repair_play_order` to bundle.rs**

In `crates/minion-presentation/src/bundle.rs`, add the import and the new public function. Insert after the existing `use` block and before `const ENTRY`:

```rust
use crate::schema::validate::validate_play_order;
```

Add after `apply_patch`:

```rust
/// Checks play_order integrity via `validate_play_order`. If validation fails,
/// rebuilds play_order from scratch by iterating `deck.all_slides()` in section order.
pub fn validate_and_repair_play_order(deck: &mut Deck) {
    if validate_play_order(deck).is_err() {
        deck.play_order = deck.all_slides().map(|s| s.id.clone()).collect();
    }
}
```

- [ ] **Step 4: Call the repair in orchestrator.rs after critic patches**

In `crates/minion-presentation/src/orchestrator.rs`, change the design-critic block (lines 86-88):

```rust
        // Design critic
        for patch in DesignCriticAgent::new().review(&deck) {
            bundle::apply_patch(&mut deck, patch);
        }
        bundle::validate_and_repair_play_order(&mut deck);
```

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cargo test -p minion-presentation bundle 2>&1 | tail -20
```

Expected:
```
test bundle_tests::delete_slide_removes_from_play_order ... ok
test bundle_tests::validate_and_repair_play_order_fixes_orphan ... ok
test bundle_tests::apply_patch_set_meta_updates_title ... ok
test bundle_tests::bundle_roundtrip_preserves_title ... ok
test bundle_tests::bundle_missing_file_errors ... ok
```

- [ ] **Step 6: Commit**

```bash
git add crates/minion-presentation/src/bundle.rs \
        crates/minion-presentation/src/orchestrator.rs \
        crates/minion-presentation/tests/bundle_tests.rs
git commit -m "fix(presentation): repair play_order on DeleteSlide patch"
```

---

## Task 2: Clippy fixes

**Context:** Two errors block compilation under `-D warnings`:
1. `visual.rs:9` — `provider: Option<Arc<dyn minion_llm::LlmProvider>>` field is never read (dead code).
2. `svg_sanitizer.rs:66` — nested `if` can be collapsed into a single condition.

**Files:**
- Modify: `crates/minion-presentation/src/agents/visual.rs`
- Modify: `crates/minion-presentation/src/visual/svg_sanitizer.rs`

- [ ] **Step 1: Fix `visual.rs` — suppress dead_code on the provider field**

The `provider` field exists to keep the `new_with_provider` constructor available for future use. Suppress with an attribute rather than removing the variant.

In `crates/minion-presentation/src/agents/visual.rs`, change:

```rust
pub struct VisualAgent {
    provider: Option<Arc<dyn minion_llm::LlmProvider>>,
}
```

to:

```rust
pub struct VisualAgent {
    #[allow(dead_code)]
    provider: Option<Arc<dyn minion_llm::LlmProvider>>,
}
```

- [ ] **Step 2: Fix `svg_sanitizer.rs` — collapse the nested if**

In `crates/minion-presentation/src/visual/svg_sanitizer.rs`, change lines 66-70:

```rust
                if (name == "href" || name == "xlink:href") && tag == "use" {
                    if !href_re().is_match(value) {
                        return Err(format!("invalid use href: {value}"));
                    }
                }
```

to:

```rust
                if (name == "href" || name == "xlink:href") && tag == "use"
                    && !href_re().is_match(value)
                {
                    return Err(format!("invalid use href: {value}"));
                }
```

- [ ] **Step 3: Verify no warnings remain**

```bash
cargo clippy -p minion-presentation -- -D warnings 2>&1 | grep "^error\|^warning\[" | head -20
```

Expected: empty output (no errors, no warnings).

- [ ] **Step 4: Run full crate tests**

```bash
cargo test -p minion-presentation 2>&1 | tail -10
```

Expected: all tests pass, no compilation errors.

- [ ] **Step 5: Commit**

```bash
git add crates/minion-presentation/src/agents/visual.rs \
        crates/minion-presentation/src/visual/svg_sanitizer.rs
git commit -m "fix(presentation): resolve clippy warnings in visual.rs and svg_sanitizer.rs"
```

---

## Task 3: Interactive HTML export upgrade

**Context:** The current `exportToHtml` in `ExportDialog.tsx` renders all slides as stacked static blocks with no navigation. Replace it with a self-contained single-file viewer: one slide visible at a time, click-through navigation, keyboard shortcuts, slide counter.

**Files:**
- Modify: `ui/src/pages/presentation/ExportDialog.tsx`

- [ ] **Step 1: Replace `exportToHtml` in ExportDialog.tsx**

The function occupies lines 34–58. Replace it entirely with the version below. The surrounding file (imports, `run`, JSX) is unchanged.

```typescript
  function exportToHtml(deck: Deck, basename: string): void {
    const slides = allSlides(deck);
    const n = slides.length;

    const slideDivs = slides.map((slide, i) => {
      const W = slide.width || 1920;
      const H = slide.height || 1080;

      const bg = slide.background.kind === "solid"
        ? colorToCss(slide.background.color)
        : slide.background.kind === "gradient"
        ? `linear-gradient(${slide.background.angle_deg}deg,${colorToCss(slide.background.from)},${colorToCss(slide.background.to)})`
        : "#0f0f14";

      const fontFamily = [
        deck.theme.typography.body.family,
        ...deck.theme.font_fallback_stack,
      ].join(",");

      const els = slide.elements
        .filter(el => el.content.kind === "text")
        .sort((a, b) => a.z_index - b.z_index)
        .map(el => {
          const md = (el.content as { kind: "text"; markdown: string }).markdown;
          const escaped = md
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/\n/g, "<br>");
          const basePx = deck.theme.typography.body.size_scale_base_px || 18;
          const relSize = (el.height / H) * basePx * 1.8;
          const textColor = colorToCss(deck.theme.color_roles.body_text);
          return `<div style="position:absolute;left:${(el.x/W*100).toFixed(2)}%;top:${(el.y/H*100).toFixed(2)}%;width:${(el.width/W*100).toFixed(2)}%;height:${(el.height/H*100).toFixed(2)}%;color:${textColor};overflow:hidden;white-space:pre-wrap;font-size:${relSize.toFixed(1)}px;line-height:${deck.theme.typography.body.line_height}">${escaped}</div>`;
        }).join("");

      const display = i === 0 ? "flex" : "none";
      return `<div class="slide" data-index="${i}" style="display:${display};position:fixed;inset:0;background:${bg};font-family:${fontFamily};align-items:center;justify-content:center;cursor:pointer"><div style="position:relative;width:min(100vw,177.78vh);height:min(56.25vw,100vh)">${els}</div><div class="counter" style="position:fixed;top:12px;right:16px;color:rgba(255,255,255,.45);font-size:13px;font-family:sans-serif;pointer-events:none">${i+1} / ${n}</div></div>`;
    }).join("\n");

    const title = deck.meta.title.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

    const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${title}</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#000;overflow:hidden}
.slide{user-select:none}
</style>
</head>
<body>
${slideDivs}
<script>
(function(){
  var idx=0,slides=document.querySelectorAll('.slide');
  var n=slides.length;
  function show(i){
    slides[idx].style.display='none';
    idx=(i+n)%n;
    slides[idx].style.display='flex';
  }
  document.addEventListener('click',function(e){
    var x=e.clientX/window.innerWidth;
    show(x<0.25?idx-1:idx+1);
  });
  document.addEventListener('keydown',function(e){
    if(e.key==='ArrowRight'||e.key===' ') show(idx+1);
    else if(e.key==='ArrowLeft') show(idx-1);
    else if(e.key==='Escape') show(0);
  });
})();
</script>
</body>
</html>`;

    const a = Object.assign(document.createElement("a"), {
      href: URL.createObjectURL(new Blob([html], { type: "text/html" })),
      download: `${basename}.html`,
    });
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }
```

- [ ] **Step 2: TypeScript type-check**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 3: ESLint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/pages/presentation/ExportDialog.tsx
git commit -m "feat(presentation): upgrade HTML export to click-through single-file viewer"
```

---

## Self-Review

**Spec coverage:**
- Fix 1 (play_order desync): covered by Task 1 — `validate_and_repair_play_order` added to `bundle.rs`, called in `orchestrator.rs` after critic loop. Test verifies both direct delete and orphan repair.
- Fix 2 (clippy warnings): covered by Task 2 — both `visual.rs` dead_code and `svg_sanitizer.rs` collapsible_if are fixed inline with exact code shown.
- Fix 3 (interactive HTML): covered by Task 3 — complete replacement function with text rendering, proportional positioning, click navigation (left quarter = back), keyboard support (ArrowLeft/Right/Space/Escape), slide counter, self-contained.

**Placeholder scan:** No TBD, TODO, or vague instructions. Every step has exact code.

**Type consistency:**
- `validate_and_repair_play_order` is defined in `bundle.rs` in Task 1 Step 3, imported in the test as `minion_presentation::bundle::validate_and_repair_play_order` in Step 1, and called in `orchestrator.rs` as `bundle::validate_and_repair_play_order(&mut deck)` in Step 4 — all consistent.
- `allSlides`, `colorToCss`, `Deck` are already imported in `ExportDialog.tsx` — no new imports needed.
