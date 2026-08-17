/** SPA entry point: mounts the root <App/> into #app. */
import { render } from 'preact';
import './styles.css';
import { App } from './app';

const root = document.getElementById('app');
if (root) {
  render(<App />, root);
}
