/* @refresh reload */
import { render } from 'solid-js/web';
import { Router, Route } from '@solidjs/router';
import App from './App';
import Dashboard from './pages/Dashboard';
import Files from './pages/Files';
import Reader from './pages/Reader';
import Finance from './pages/Finance';
import Fitness from './pages/Fitness';
import Health from './pages/Health';
import Media from './pages/Media';
import Calendar from './pages/Calendar';
import Blog from './pages/Blog';
import Sysmon from './pages/Sysmon';
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
      <Route path="/health" component={Health} />
      <Route path="/media" component={Media} />
      <Route path="/calendar" component={Calendar} />
      <Route path="/blog" component={Blog} />
      <Route path="/sysmon" component={Sysmon} />
      <Route path="/settings" component={Settings} />
    </Router>
  ),
  root
);
