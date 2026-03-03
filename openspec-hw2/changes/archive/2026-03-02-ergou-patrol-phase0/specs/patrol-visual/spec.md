## ADDED Requirements

### Requirement: SVG paw asset with correct anatomy

The system SHALL provide a single-paw SVG with viewBox 16×16, consisting of 4 toe pad ellipses and 1 palm pad ellipse, using `fill="currentColor"` for color inheritance. File size SHALL be under 500 bytes.

- Toes SHALL point UP in the default orientation (0° heading)
- Right foot SHALL be the left foot with scaleX(-1), no separate SVG needed

#### Scenario: SVG renders at correct size
- **WHEN** paw SVG is placed in a 32×32 container
- **THEN** it SHALL render proportionally with visible toe and palm pads

#### Scenario: Color inheritance
- **WHEN** parent element has `color: red`
- **THEN** paw SVG fill SHALL be red via currentColor

---

### Requirement: Tab icon SVG supports fill-to-stroke transition

The system SHALL provide a dual-paw SVG for the tab icon with separate `.paw-fill` and `.paw-stroke` groups, enabling CSS-driven transitions between filled (home) and outlined (patrol-out) states.

- `.paw-fill` group SHALL use `fill: currentColor` and transition opacity
- `.paw-stroke` group SHALL use `stroke: currentColor, fill: none` and default to opacity 0
- The SVG SHALL replace emoji 🐾 in three locations: mobile-nav-abao icon, abao-avatar, abao-mini-avatar

#### Scenario: Home state (solid)
- **WHEN** tab icon has no patrol state class
- **THEN** .paw-fill SHALL be opacity 1 and .paw-stroke SHALL be opacity 0

#### Scenario: Patrol-out state (outline)
- **WHEN** tab icon has class `patrol-out`
- **THEN** .paw-fill SHALL be opacity 0 and .paw-stroke SHALL be opacity 0.3

#### Scenario: Pulse on return
- **WHEN** tab icon has class `patrol-pulse`
- **THEN** icon SHALL animate scale 1→1.15→1 over 0.3s

---

### Requirement: Heading system rotates paws to match travel direction

The system SHALL use CSS custom properties `--paw-heading` and `--paw-mirror` to compose heading rotation with landing/evaporate scale animations without conflict.

- `--paw-heading` SHALL specify the direction of travel (0deg=up, 90deg=right, 180deg=down, 270deg=left)
- `--paw-mirror` SHALL be 1 (left foot) or -1 (right foot)
- All keyframe animations (landing, evaporate) SHALL include `scaleX(var(--paw-mirror)) rotate(var(--paw-heading))` in their transform chains
- Ripple pseudo-elements (::before, ::after) SHALL NOT be affected by heading rotation (they use their own translate-based transforms)

#### Scenario: Walking right
- **WHEN** heading is 90deg
- **THEN** paw toes SHALL point to the right of the screen

#### Scenario: Turning from right to down
- **WHEN** heading transitions from 90deg to 180deg across successive steps
- **THEN** paw orientation SHALL smoothly rotate from right-facing to downward-facing

#### Scenario: Landing animation preserves heading
- **WHEN** a paw with heading=90deg plays the landing animation
- **THEN** the scale bounce SHALL occur while maintaining the 90deg rotation

---

### Requirement: Paw landing produces elastic bounce and ripple

Each paw print SHALL play a landing animation on placement: scale 0→1.1→1 over 0.15s with two concentric ripple rings expanding from center.

- Ripple ring 1: scale 0.3→2.5, opacity 0.4→0 over 0.6s
- Ripple ring 2: same with 0.1s delay
- Ripple color SHALL inherit via currentColor

#### Scenario: Landing animation sequence
- **WHEN** a paw element receives class 'landing'
- **THEN** it SHALL scale from 0 to 1.1 to 1 with opacity 0→0.5, and two ripple rings SHALL expand outward

---

### Requirement: Paw evaporation mimics water stain fading

Each paw print SHALL evaporate after its display period: opacity 0.5→0 and scale 1→0.95 over 1.5s with ease-in timing.

#### Scenario: Evaporation after display
- **WHEN** paw receives class 'evaporate'
- **THEN** opacity SHALL fade from 0.5 to 0 and scale SHALL shrink to 0.95 over 1.5s

---

### Requirement: Enhanced mode uses mix-blend-mode on high-end devices

When `#patrol-layer` has class `patrol-enhanced`, paw prints SHALL use `mix-blend-mode: soft-light` to blend with underlying content. Without the class, paws SHALL use fixed semi-transparency.

#### Scenario: High-end device rendering
- **WHEN** patrol-layer has class patrol-enhanced
- **THEN** .patrol-paw elements SHALL have mix-blend-mode: soft-light

#### Scenario: Low-end device rendering
- **WHEN** patrol-layer does NOT have class patrol-enhanced
- **THEN** .patrol-paw elements SHALL use opacity only, no blend mode

---

### Requirement: Reduced motion disables all patrol animations

When `prefers-reduced-motion: reduce` is active, the patrol layer SHALL be hidden and all patrol animations SHALL be disabled via CSS.

#### Scenario: Reduced motion active
- **WHEN** user has prefers-reduced-motion: reduce
- **THEN** #patrol-layer SHALL have display: none and all patrol animations SHALL be set to animation: none
