# Requirements: Preact SPA Frontend

<!-- spec-nav:start -->
**Spec navigation:** [State](00_state.md) · [Discovery](01_discovery.md) · [Requirements](02_requirements.md) · [Design](03_design.md) · [Tasks](04_tasks.md)
<!-- spec-nav:end -->

Replace MoodLightPi's hand-written vanilla-JS web UI with a component-based Preact + TypeScript
single-page app, preserving every current capability and the OctoCam-style layout, while keeping
the appliance a single self-contained binary. These requirements describe **observable behavior**;
technology and file-layout choices are deferred to [03_design.md](03_design.md).

Scope, chosen approach, and the full parity inventory are fixed by the approved
[`01_discovery.md`](01_discovery.md).

## Terms

- **Web UI** — the browser-side single-page app served by the device.
- **Server** — the device's HTTP/WebSocket service ([`src/api.rs`](../../src/api.rs), etc.).
- **Build** — producing the deployable binary with the web UI embedded.
- **Owner** — a person on the local network controlling or configuring the device.
- **Bundle** — the built web-UI assets embedded into the binary.
- **Bootstrap** — a single response aggregating the data the Web UI needs at initial load.

## Assumptions

- Device runtime behavior (engine, effects, persistence, hardware, HomeKit runtime) is unchanged;
  the frontend and the HTTP-serving/aggregation layer change, and the MQTT runtime gains
  availability advertising and Home Assistant discovery (R17–R18).
- The Web UI is served from and talks to the same device origin on the LAN.
- Existing endpoint request/response shapes remain the contract except where a requirement below
  explicitly introduces a change.

---

### Requirement 1: Light control parity

**User Story:** As an owner, I want to set color, brightness, and effect with speed from the web UI, so that I retain every lighting control the previous UI offered.

1. **R1.1** WHEN the owner selects a color via the color picker or a preset swatch, THE Web UI SHALL send the chosen red, green, and blue values to the Server and display the selected color.
2. **R1.2** WHEN the owner changes the brightness control, THE Web UI SHALL send the corresponding brightness value in the range 0 to 255 and display the level as a percentage from 0 to 100.
3. **R1.3** WHEN the owner selects an effect, THE Web UI SHALL send the selected effect name together with the speed value when the effect uses speed, and display the current effect.
4. **R1.4** WHILE the selected effect is the solid effect, THE Web UI SHALL hide the speed control.
5. **R1.5** WHEN the owner moves the brightness or speed control continuously, THE Web UI SHALL coalesce updates so that no more than one value is sent per 80 milliseconds of control inactivity.
6. **R1.6** THE Web UI SHALL populate the selectable effect list from the effect set the Server advertises rather than a hard-coded list.

### Requirement 2: Live LED preview

**User Story:** As an owner, I want a live preview of the LED panel, so that I can see the current output without looking at the physical light.

1. **R2.1** WHEN the dashboard is shown, THE Web UI SHALL open a WebSocket to the Server and render each received frame as the panel's 32 LED colors in an 8-by-4 grid.
2. **R2.2** THE Web UI SHALL render the preview grid in the same orientation as the physical panel, reversed along both the horizontal and vertical axes.
3. **R2.3** IF the preview WebSocket connection closes, THEN THE Web UI SHALL attempt to reopen it within approximately 1 second.

### Requirement 3: Device reachability status

**User Story:** As an owner, I want to see whether the device is reachable, so that I can trust what the UI shows.

1. **R3.1** WHILE the Server responds successfully to health checks, THE Web UI SHALL display an active status indicator.
2. **R3.2** IF a health check fails, THEN THE Web UI SHALL display an offline status indicator.
3. **R3.3** THE Web UI SHALL refresh the displayed light state at least once every 5 seconds so the controls stay consistent with the device.

### Requirement 4: Identity settings

**User Story:** As an owner, I want to name my device and light, so that it is identifiable across the UI and integrations.

1. **R4.1** WHEN the owner opens the identity settings, THE Web UI SHALL display the current device name, room, and light label.
2. **R4.2** WHEN the owner saves identity settings with a non-empty device name and light label, THE Web UI SHALL send the values to the Server and display the saved result.
3. **R4.3** IF the owner attempts to save identity settings with an empty device name or empty light label, THEN THE Server SHALL reject the request with a client error and THE Web UI SHALL surface the error.

### Requirement 5: Wi-Fi settings

**User Story:** As an owner, I want to configure Wi-Fi and pick from nearby networks, so that I can move the device onto my network from the UI.

1. **R5.1** WHEN the owner opens the Wi-Fi settings, THE Web UI SHALL display the current SSID, security type, autoconnect preference, whether a password is already set, and the connected network when known.
2. **R5.2** WHEN the owner requests a scan, THE Web UI SHALL request nearby networks from the Server and present them as selectable SSID choices.
3. **R5.3** WHEN the owner saves Wi-Fi settings, THE Web UI SHALL send the SSID, password, security type, and autoconnect preference to the Server and display the resulting status.
4. **R5.4** IF a Wi-Fi scan cannot be performed, THEN THE Server SHALL return an error and THE Web UI SHALL surface it without discarding the entered SSID.

### Requirement 6: MQTT settings

**User Story:** As an owner, I want to configure an MQTT broker connection, so that the light integrates with my home automation.

1. **R6.1** WHEN the owner opens the MQTT settings, THE Web UI SHALL display the enabled state, broker URL, client id, username, subscribe topic, publish topic, and whether a password is already stored, without revealing the stored password.
2. **R6.2** WHEN the owner saves MQTT settings without entering a new password, THE Web UI SHALL preserve the previously stored password.
3. **R6.3** WHEN the owner saves MQTT settings with the clear-password option selected, THE Server SHALL remove any stored password.
4. **R6.4** IF the owner saves MQTT settings that are invalid, THEN THE Server SHALL reject them with a client error and THE Web UI SHALL surface the reason.
5. **R6.5** WHEN the owner opens the MQTT settings, THE Web UI SHALL display the availability topic, whether Home Assistant discovery is enabled, and the Home Assistant discovery prefix.
6. **R6.6** WHEN the owner saves MQTT settings, THE Web UI SHALL send the availability topic, the discovery enabled state, and the discovery prefix together with the other MQTT fields.

### Requirement 7: HomeKit settings

**User Story:** As an owner, I want to enable HomeKit and see a pairing code, so that I can add the light to the Home app.

1. **R7.1** WHEN the owner opens the HomeKit settings, THE Web UI SHALL display the accessory name, the enabled state, and the current pairing status.
2. **R7.2** WHILE HomeKit is enabled and ready to pair but not yet paired, THE Web UI SHALL display the pairing code.
3. **R7.3** WHEN the owner saves HomeKit settings with a non-empty accessory name, THE Web UI SHALL send the values to the Server and display the updated status.
4. **R7.4** IF the owner attempts to save HomeKit settings with an empty accessory name, THEN THE Server SHALL reject the request with a client error and THE Web UI SHALL surface the error.

### Requirement 8: SSH authorized keys

**User Story:** As an owner, I want to manage SSH authorized keys, so that I can control remote access to the device.

1. **R8.1** WHEN the owner opens the SSH settings, THE Web UI SHALL display the currently authorized keys.
2. **R8.2** WHEN the owner requests validation of entered keys, THE Web UI SHALL report whether the keys are valid.
3. **R8.3** WHEN the owner saves valid authorized keys, THE Web UI SHALL send them to the Server and reflect the saved result.
4. **R8.4** IF the owner saves keys that fail validation, THEN THE Server SHALL reject them with a client error and THE Web UI SHALL surface the error.

### Requirement 9: System power actions

**User Story:** As an owner, I want to restart the service, reboot, or power off the device, so that I can manage it without a shell.

1. **R9.1** WHEN the owner confirms a restart-service, reboot, or poweroff action in the system-power dialog, THE Web UI SHALL request that action from the Server and confirm that it was queued.
2. **R9.2** IF a system action other than restart-service, reboot, or poweroff is requested, THEN THE Server SHALL reject it with a client error.
3. **R9.3** WHEN the owner cancels the system-power dialog or presses Escape, THE Web UI SHALL close the dialog without requesting any action.

### Requirement 10: Client-side navigation

**User Story:** As an owner, I want to move between the dashboard and settings sections with shareable URLs, so that navigation feels like a normal web app.

1. **R10.1** THE Web UI SHALL provide client-side navigation among the dashboard and the identity, Wi-Fi, MQTT, HomeKit, SSH, and device settings sections, each reachable at a distinct URL path.
2. **R10.2** WHEN a browser requests any of the Web UI's client route paths directly, THE Server SHALL respond with the SPA entry document so the requested view renders.
3. **R10.3** WHEN the owner uses browser back or forward navigation, THE Web UI SHALL display the view corresponding to the resulting URL.

### Requirement 11: Aggregate bootstrap

**User Story:** As an owner on a low-powered device, I want the UI to load in a single round-trip, so that it appears quickly.

1. **R11.1** WHEN the Web UI requests the bootstrap resource, THE Server SHALL return, in one response, the current light state, the available effect set, and the identity, Wi-Fi, MQTT, HomeKit, and SSH settings.
2. **R11.2** THE field values in the bootstrap response SHALL equal the values the corresponding individual endpoints return for the same state.
3. **R11.3** THE Web UI SHALL complete its initial data load using the bootstrap resource without issuing separate per-section requests for that initial render.

### Requirement 12: Asset serving and SPA fallback

**User Story:** As an owner, I want the site to load reliably regardless of which URL I open, so that bookmarks and refreshes work.

1. **R12.1** WHEN a browser requests a bundled asset path, THE Server SHALL respond with that asset's content and a matching content type.
2. **R12.2** WHEN a browser requests a path that is not an API route, not the WebSocket route, and not a bundled asset, THE Server SHALL respond with the SPA entry document.
3. **R12.3** THE Server SHALL continue to route API and WebSocket requests to their handlers rather than serving the SPA entry document in their place.
4. **R12.4** IF a browser requests a bundled asset path that does not exist, THEN THE Server SHALL respond with a not-found status rather than the SPA entry document.

### Requirement 13: Self-contained build without Node on the device

**User Story:** As a maintainer, I want the device build to need only a Rust toolchain, so that the Pi never needs a Node toolchain.

1. **R13.1** WHEN the project is built with only a Rust toolchain present and no Node or npm available, THE Build SHALL succeed and produce a binary that serves the complete web UI.
2. **R13.2** THE running Server SHALL serve the entire web UI from the single binary without reading web assets from disk at runtime.
3. **R13.3** IF the committed Bundle does not match what the frontend source would currently produce, THEN THE Build verification SHALL report the discrepancy.

### Requirement 14: Security parity

**User Story:** As an owner, I want the device to stay LAN-only, so that the rewrite does not widen its exposure.

1. **R14.1** THE Web UI SHALL issue every API and WebSocket request to the same origin from which it was served.
2. **R14.2** IF an API or WebSocket request presents a non-LAN Host header or a foreign Origin header, THEN THE Server SHALL reject it with a forbidden response.

### Requirement 15: Footprint

**User Story:** As a maintainer, I want the new UI to stay small, so that it remains suitable for an embedded device.

1. **R15.1** THE compressed JavaScript and CSS of the Bundle together SHALL be no larger than 50 kilobytes.

### Requirement 16: User feedback

**User Story:** As an owner, I want clear feedback on my actions, so that I know whether a change took effect.

1. **R16.1** WHEN a settings save or system action succeeds, THE Web UI SHALL show a transient confirmation message.
2. **R16.2** IF an API request fails, THEN THE Web UI SHALL show a transient message describing the failure.

### Requirement 17: MQTT availability advertising

**User Story:** As an owner using Home Assistant, I want the light to report whether it is online, so that Home Assistant shows the entity's true availability.

1. **R17.1** WHEN the Server establishes its MQTT broker connection, THE Server SHALL register a Last-Will message that publishes an offline payload to the configured availability topic if the connection is lost.
2. **R17.2** WHILE the Server is connected to the broker with MQTT enabled, THE Server SHALL publish an online payload to the configured availability topic.
3. **R17.3** THE Server SHALL publish availability messages with the retain flag set so a subscriber receives the current availability immediately on connecting.
4. **R17.4** WHEN MQTT is disabled or the Server shuts down cleanly, THE Server SHALL publish an offline availability payload before disconnecting.

### Requirement 18: Home Assistant MQTT discovery

**User Story:** As an owner, I want the light to register itself in Home Assistant automatically, so that I do not configure the entity by hand.

1. **R18.1** WHILE MQTT and Home Assistant discovery are both enabled, THE Server SHALL publish a retained discovery configuration for the light under the configured discovery prefix so that Home Assistant creates the light entity automatically.
2. **R18.2** THE discovery configuration SHALL include a stable unique identifier, a device block, and references to the light's command, state, and availability topics.
3. **R18.3** THE discovery configuration SHALL describe the light's controllable attributes of on/off, color, and brightness consistent with the device's capabilities.
4. **R18.4** WHEN MQTT or Home Assistant discovery is disabled, THE Server SHALL clear the previously published discovery configuration so that Home Assistant no longer offers a stale entity.

---

## Open Questions

None outstanding. Endpoint-shape cleanup details (exact bootstrap payload, whether unused
endpoints are pruned or kept) are design decisions bounded by R11 and R12 and are resolved in
[03_design.md](03_design.md).

## Approval

Status: **Approved on 2026-08-16**
