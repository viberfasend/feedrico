import '@fontsource-variable/inter';
import '@fontsource-variable/literata';
import './lib/tokens/index.css';
import './app.css';

import { mount } from 'svelte';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('#app missing from index.html');

export default mount(App, { target });
