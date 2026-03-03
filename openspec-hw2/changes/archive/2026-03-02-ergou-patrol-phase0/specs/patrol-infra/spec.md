## ADDED Requirements

### Requirement: ObjectPool manages DOM elements with fixed pool size

The system SHALL provide a generic DOM object pool that pre-creates elements and reuses them via acquire/release, avoiding runtime DOM creation and destruction.

- Pool size SHALL be configurable at creation time
- `acquire()` SHALL return an idle element or null if pool exhausted
- `release(el)` SHALL call a user-defined reset function and hide the element
- `releaseAll()` SHALL release all active elements at once
- `destroy()` SHALL remove all DOM elements from the document
- `activeCount` SHALL return the current number of acquired elements

#### Scenario: Acquire and release cycle
- **WHEN** pool is created with size 8 and 3 elements are acquired
- **THEN** activeCount SHALL be 3 and acquire() SHALL return a non-null element

#### Scenario: Pool exhaustion
- **WHEN** all 8 elements are acquired and acquire() is called
- **THEN** acquire() SHALL return null

#### Scenario: Release resets element
- **WHEN** an element is released
- **THEN** the reset function SHALL be called and the element SHALL be hidden (display:none)

---

### Requirement: DeviceProfile detects device capabilities at load time

The system SHALL detect device capabilities once at module load and expose tier, blend support, reduced-motion preference, and browser support.

- `tier` SHALL be 'high' if `navigator.deviceMemory >= 4` OR `navigator.hardwareConcurrency >= 4`, otherwise 'low'
- `canBlend` SHALL be true when tier is 'high'
- `reduceMotion` SHALL be true when `prefers-reduced-motion: reduce` matches
- `isSupported` SHALL be true when browser supports requestAnimationFrame, classList, and CSS transform
- The system SHALL listen for reduced-motion media query changes and invoke a callback

#### Scenario: High-end device
- **WHEN** navigator.hardwareConcurrency is 8
- **THEN** tier SHALL be 'high' and canBlend SHALL be true

#### Scenario: Low-end device
- **WHEN** navigator.deviceMemory is 2 and navigator.hardwareConcurrency is 2
- **THEN** tier SHALL be 'low' and canBlend SHALL be false

#### Scenario: Reduced motion enabled
- **WHEN** user has prefers-reduced-motion: reduce set in OS
- **THEN** reduceMotion SHALL be true

#### Scenario: APIs unavailable
- **WHEN** deviceMemory and hardwareConcurrency both return undefined
- **THEN** tier SHALL be 'low'

---

### Requirement: CSSAnimator manages runtime CSS keyframe injection

The system SHALL create a dedicated `<style>` element and provide methods to inject, remove, and clear `@keyframes` rules at runtime.

- `inject(name, keyframesCSS)` SHALL insert a @keyframes rule into the stylesheet
- Injecting a name that already exists SHALL replace the existing rule
- `remove(name)` SHALL delete the named @keyframes rule
- `clear()` SHALL remove all injected rules
- `generateWalkKeyframes(points)` SHALL accept an array of `{x, y, rotate}` and return a CSS keyframes string with percentage-based steps

#### Scenario: Inject and remove keyframes
- **WHEN** inject('walk-1', '0%{...} 100%{...}') is called
- **THEN** the stylesheet SHALL contain a @keyframes rule named 'walk-1'
- **WHEN** remove('walk-1') is called
- **THEN** the rule SHALL no longer exist in the stylesheet

#### Scenario: Generate walk keyframes from points
- **WHEN** generateWalkKeyframes([{x:0,y:0,rotate:0}, {x:100,y:50,rotate:45}]) is called
- **THEN** the output SHALL be a string with '0%' and '100%' stops containing translate and rotate transforms

---

### Requirement: IdleDetector monitors user activity with cooldown

The system SHALL detect user idle state (no interaction for a configurable threshold) and manage a cooldown period after dismissal.

- `idleThreshold` SHALL default to 8000ms and be adjustable at runtime
- `cooldown` SHALL default to 180000ms (3 minutes) and be adjustable at runtime
- `onIdle` callback SHALL fire when idle threshold reached AND cooldown has elapsed
- `onActive` callback SHALL fire when user resumes interaction during idle state
- Active events SHALL include click, scroll, keydown, touchstart (passive listeners)
- When document becomes hidden (visibilitychange), idle timer SHALL be cleared
- `startCooldown()` SHALL set cooldown end time and prevent idle triggers until elapsed
- `resetCooldown()` SHALL clear cooldown immediately (debug use)

#### Scenario: Normal idle trigger
- **WHEN** user has not interacted for 8 seconds and cooldown is not active
- **THEN** onIdle SHALL be called

#### Scenario: Idle during cooldown
- **WHEN** user has not interacted for 8 seconds but cooldown has 2 minutes remaining
- **THEN** onIdle SHALL NOT be called until cooldown expires

#### Scenario: User becomes active during idle
- **WHEN** idle state is active and user clicks
- **THEN** onActive SHALL be called and idle state SHALL be reset

#### Scenario: Tab hidden
- **WHEN** document.hidden becomes true during idle timer countdown
- **THEN** idle timer SHALL be cleared and no idle event SHALL fire

---

### Requirement: PatrolStateMachine enforces valid state transitions

The system SHALL implement a state machine with states: home, peek, walk, pause, rest, converge. Transitions SHALL only occur on valid event+state combinations per the transition table.

- Valid transitions:
  - home + idle → peek (requires cooldown elapsed)
  - peek + peekDone → walk
  - peek + click/scroll/modal → home
  - walk + click → home
  - walk + scroll → home
  - walk + modal → home
  - walk + abaoTab → converge
  - walk + walkEnd → pause
  - pause + click/scroll/modal → home
  - pause + abaoTab → converge
  - pause + pauseTimeout → walk
  - pause + restTimeout → rest
  - rest + click/scroll/modal → home
  - rest + abaoTab → converge
  - rest + idle → walk
  - converge + convergeDone → home
- Invalid transitions SHALL return false and not change state
- `canPatrol` SHALL return true when state is not home and not converge
- `forceState(state)` SHALL bypass transition table (debug use)
- `reset()` SHALL force state to home

#### Scenario: Normal patrol cycle
- **WHEN** events occur in sequence: idle, peekDone, walkEnd
- **THEN** states SHALL transition: home → peek → walk → pause

#### Scenario: Walk interrupted by click
- **WHEN** state is walk and click event occurs
- **THEN** state SHALL transition to home

#### Scenario: Invalid transition ignored
- **WHEN** state is home and peekDone event occurs
- **THEN** transition SHALL return false and state SHALL remain home

#### Scenario: Converge cycle
- **WHEN** state is walk and abaoTab event occurs, then convergeDone
- **THEN** states SHALL transition: walk → converge → home

---

### Requirement: PawPool places and evaporates paw prints

The system SHALL manage an ObjectPool of 8 paw print elements with heading-aware placement, landing animation, and automatic evaporation.

- `step(x, y, isLeft, color, heading)` SHALL acquire a pool element, set CSS custom properties `--paw-heading` and `--paw-mirror`, position it, and add the 'landing' class
- Right foot (`isLeft=false`) SHALL set `--paw-mirror: -1` and negate the heading to compensate for scaleX(-1)
- Each paw SHALL automatically evaporate (fade + shrink) after 800ms and be released back to the pool after evaporation completes
- `fadeAll(duration)` SHALL release all active paws
- `clear()` SHALL cancel all evaporation timers and release all paws immediately

#### Scenario: Place a left paw walking right
- **WHEN** step(100, 200, true, '#333', 90) is called
- **THEN** the element SHALL have --paw-mirror: 1, --paw-heading: 90deg, class 'patrol-paw left landing', and be positioned at (84, 184)

#### Scenario: Place a right paw walking right
- **WHEN** step(100, 200, false, '#333', 90) is called
- **THEN** the element SHALL have --paw-mirror: -1, --paw-heading: -90deg (negated for scaleX compensation)

#### Scenario: Automatic evaporation
- **WHEN** a paw is placed
- **THEN** after 800ms it SHALL transition to 'evaporate' class and after 1500ms more it SHALL be released to the pool
