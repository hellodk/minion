import { Component, createSignal, onMount, ParentProps } from 'solid-js';
import Layout from './components/Layout';
import { invoke } from '@tauri-apps/api/core';

export interface SystemInfo {
  version: string;
  platform: string;
  arch: string;
  data_dir: string;
}

const App: Component<ParentProps> = (props) => {
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo | null>(null);
  const [darkMode, setDarkMode] = createSignal(false);

  onMount(async () => {
    // Get system info from backend
    try {
      const info = await invoke<SystemInfo>('get_system_info');
      setSystemInfo(info);
    } catch (e) {
      console.error('Failed to get system info:', e);
    }

    // Check for dark mode preference
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    setDarkMode(prefersDark);
    document.documentElement.classList.toggle('dark', prefersDark);
  });

  const toggleDarkMode = () => {
    const newValue = !darkMode();
    setDarkMode(newValue);
    document.documentElement.classList.toggle('dark', newValue);
  };

  return (
    <Layout systemInfo={systemInfo()} darkMode={darkMode()} toggleDarkMode={toggleDarkMode}>
      {props.children}
    </Layout>
  );
};

export default App;
