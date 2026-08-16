/** Settings shell: a left nav plus the active section, chosen from the current URL path (R10.1).
 * Section paths accept both the bare (`/mqtt`) and `/settings/*` alias forms and the `/ssh` /
 * `/ssh-keys` aliases, matching the previous UI's routes. */
import { navigate, useRoute } from '../router';
import { IdentityForm } from './IdentityForm';
import { WifiForm } from './WifiForm';
import { MqttForm } from './MqttForm';
import { HomeKitForm } from './HomeKitForm';
import { SshForm } from './SshForm';
import { DevicePanel } from './DevicePanel';

type Section = 'identity' | 'wifi' | 'mqtt' | 'homekit' | 'ssh' | 'device';

const NAV: { section: Section; label: string; path: string }[] = [
  { section: 'identity', label: 'Identity', path: '/identity' },
  { section: 'wifi', label: 'Wi-Fi', path: '/wifi' },
  { section: 'mqtt', label: 'MQTT', path: '/mqtt' },
  { section: 'homekit', label: 'HomeKit', path: '/homekit' },
  { section: 'ssh', label: 'SSH keys', path: '/ssh' },
  { section: 'device', label: 'Device', path: '/device' },
];

/** Map a URL path to the settings section it selects (defaults to identity). */
export function sectionFromPath(path: string): Section {
  const tail = path.replace(/^\/settings/, '').replace(/^\//, '') || 'identity';
  if (tail === 'ssh' || tail === 'ssh-keys') return 'ssh';
  if (tail === 'wifi') return 'wifi';
  if (tail === 'mqtt') return 'mqtt';
  if (tail === 'homekit') return 'homekit';
  if (tail === 'device') return 'device';
  return 'identity';
}

export function SettingsLayout() {
  const { path } = useRoute();
  const active = sectionFromPath(path);

  return (
    <main class="settings-layout">
      <nav class="settings-nav">
        {NAV.map((item) => (
          <a
            key={item.section}
            href={item.path}
            class={`nav-item ${active === item.section ? 'active' : ''}`}
            onClick={(e) => {
              e.preventDefault();
              navigate(item.path);
            }}
          >
            {item.label}
          </a>
        ))}
      </nav>
      <div class="settings-workspace">
        {active === 'identity' && <IdentityForm />}
        {active === 'wifi' && <WifiForm />}
        {active === 'mqtt' && <MqttForm />}
        {active === 'homekit' && <HomeKitForm />}
        {active === 'ssh' && <SshForm />}
        {active === 'device' && <DevicePanel />}
      </div>
    </main>
  );
}
