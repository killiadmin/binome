import { createApp } from 'vue';
import { createPinia } from 'pinia'
import App from './App.vue';
import router from './router';
import echo from './ressources/js/echo'

import * as BootstrapVueNext from 'bootstrap-vue-next';
import 'bootstrap/dist/css/bootstrap.css';
import 'bootstrap-vue-next/dist/bootstrap-vue-next.css';
import '@fortawesome/fontawesome-free/css/all.css';
import './style.css';

const app = createApp(App);
const pinia = createPinia();

for (const [key, component] of Object.entries(BootstrapVueNext)) {
    app.component(key, component);
}

app.use(pinia)
app.use(router);
app.provide('echo', echo)

app.mount('#app');
