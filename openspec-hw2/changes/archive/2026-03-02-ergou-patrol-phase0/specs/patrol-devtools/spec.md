## ADDED Requirements

### Requirement: Animation showcase provides isolated testing environment

The system SHALL provide a standalone HTML page (patrol-showcase.html) that loads patrol-utils.js and patrol.css without depending on the main application. The page SHALL include interactive demo sections for all visual elements.

- Showcase SHALL include sections for: paw rendering, landing animation, evaporate animation, gait preview (slow/normal/trot), path tests (straight/turn/arc/circle/S-curve/zigzag/spiral), exit animations, blend mode comparison, tab icon transitions, device simulation, state machine visualization, performance benchmark, state machine unit tests
- Each section SHALL have replay buttons and parameter sliders where applicable
- Path test demos SHALL draw a reference path line (dashed) and place paw prints with correct heading along the path
- Bottom FPS counter SHALL display real-time frame rate

#### Scenario: Path test with heading
- **WHEN** developer clicks "弧线" (arc) in path tests
- **THEN** paw prints SHALL be placed along a half-ellipse arc with toes pointing along the path tangent at each step

#### Scenario: Gait parameter adjustment
- **WHEN** developer adjusts stride slider from 40px to 60px
- **THEN** gait preview SHALL re-render with wider spacing between paw prints

---

### Requirement: Performance benchmark measures frame budget compliance

The showcase SHALL include a benchmark that automatically runs a 10-second patrol sequence (spawn → walk 20 steps → despawn) and reports frame timing statistics.

- Benchmark SHALL run in both 'high' and 'low' device modes
- Output SHALL include: frame count, avg/p95/p99 frame time, over-budget count and percentage
- Pass criteria: p99 < 2ms AND over-budget < 5%

#### Scenario: Benchmark passes
- **WHEN** benchmark completes with p99 = 0.5ms and over-budget = 2%
- **THEN** result SHALL display "PASS"

#### Scenario: Benchmark fails
- **WHEN** benchmark completes with over-budget = 8%
- **THEN** result SHALL display "FAIL" with details on which criterion failed

---

### Requirement: State machine unit tests cover all transition paths

The showcase SHALL include a test runner that validates PatrolStateMachine against all transition paths defined in the spec.

- Tests SHALL cover: normal cycle (home→peek→walk→home), all interrupt paths (click/scroll/modal from walk/peek/pause/rest), converge cycle, pause/rest sub-states, invalid transitions, canPatrol property, forceState, reset
- Test output SHALL show pass/fail per case and summary count

#### Scenario: All tests pass
- **WHEN** developer clicks "Run Tests" and all transitions are correct
- **THEN** output SHALL show green text with "N/N passed"

#### Scenario: Test failure
- **WHEN** a transition is incorrect
- **THEN** output SHALL show red text with the failing test name and "FAIL" marker

---

### Requirement: Debug panel provides runtime inspection and control

The system SHALL provide a debug panel (patrol-debug.js) that activates when `localStorage.getItem('patrol-debug') === '1'` and displays over the main application.

- Panel SHALL display: current state, position, platform ID, paw pool occupancy (active/8), cooldown remaining, device tier, FPS, frame avg/peak, over-budget percentage
- Panel SHALL provide buttons: Force Out (skip idle→peek→walk), Force Home (any→home), Pause/Resume, Reset Cooldown, Toggle Terrain Overlay
- Panel SHALL provide parameter sliders: idle threshold (1-30s), cooldown (0-10min), paw opacity (0.1-1.0), speed (10-100px/s), paw size (8-32px)
- All slider changes SHALL take effect immediately via CustomEvent dispatch
- Panel SHALL NOT exist in DOM when debug mode is off

#### Scenario: Enable debug panel
- **WHEN** localStorage has 'patrol-debug' = '1' and page loads
- **THEN** debug panel SHALL appear at bottom-right with live metrics

#### Scenario: Force out
- **WHEN** developer clicks "Force Out"
- **THEN** state machine SHALL transition from home to peek, then to walk after 300ms

#### Scenario: Parameter adjustment
- **WHEN** developer drags paw opacity slider to 0.3
- **THEN** a CustomEvent 'patrol:debugParam' SHALL fire with {key:'opacity', value:0.3}

---

### Requirement: Showcase is mobile-first with desktop preview mode

The showcase page SHALL use a mobile-first responsive layout. On screens wider than 768px, a notice SHALL inform the developer that the patrol system targets mobile only.

- Default layout: single column, no max-width constraint
- Below 540px: compact padding (8px) and smaller fonts
- Above 768px: max-width 960px with desktop hint banner visible

#### Scenario: Mobile view
- **WHEN** viewport width is 375px
- **THEN** showcase SHALL display in single column with 8px padding

#### Scenario: Desktop view
- **WHEN** viewport width is 1024px
- **THEN** showcase SHALL display centered at max 960px with yellow notice banner reading "桌面端预览模式"
