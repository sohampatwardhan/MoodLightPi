/**
 * SPA entry point: mounts the root component into #app.
 *
 * This is the placeholder scaffold (task 1.1); the real <App/> — router shell,
 * bootstrap load, dashboard, and settings — is wired in task 4.1.
 */
import { render } from 'preact';
import './styles.css';

function App() {
  return <div id="app-root">MoodLightPi</div>;
}

const root = document.getElementById('app');
if (root) {
  render(<App />, root);
}
