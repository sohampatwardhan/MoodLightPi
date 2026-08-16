/** Device info panel — informational parity with the previous UI's mostly-static Device section.
 * The actionable device controls live in the system-power dialog (header) and the other settings
 * sections; this panel surfaces where the UI is being served from. */
export function DevicePanel() {
  return (
    <section class="card">
      <h2>Device</h2>
      <p class="hint">This device's web UI is served from the address below.</p>
      <label class="field"><span>Address</span>
        <input type="text" value={location.host} readOnly /></label>
      <p class="status-line">Use the Power button in the header to restart the service, reboot, or power off.</p>
    </section>
  );
}
