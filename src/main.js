import { createApp } from 'vue';
import App from './App.vue';
import router from './router';

import * as BootstrapVueNext from 'bootstrap-vue-next';
import 'bootstrap/dist/css/bootstrap.css';
import 'bootstrap-vue-next/dist/bootstrap-vue-next.css';
import './style.css';

const app = createApp(App);

for (const [key, component] of Object.entries(BootstrapVueNext)) {
    app.component(key, component);
}

app.use(router);
app.mount('#app');
