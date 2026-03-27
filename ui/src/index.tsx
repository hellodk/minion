/* @refresh reload */
import { render } from 'solid-js/web';
import { Router, Route } from '@solidjs/router';
import App from './App';
import Dashboard from './pages/Dashboard';
import Files from './pages/Files';
import Reader from './pages/Reader';
import Finance from './pages/Finance';
import Fitness from './pages/Fitness';
import Settings from './pages/Settings';
import './styles/index.css';

const root = document.getElementById('root');

if (!root) {
  throw new Error('Root element not found');
}

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Dashboard} />
      <Route path="/files" component={Files} />
      <Route path="/reader" component={Reader} />
      <Route path="/finance" component={Finance} />
      <Route path="/fitness" component={Fitness} />
      <Route path="/settings" component={Settings} />
    </Router>
  ),
  root
);
