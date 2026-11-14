import { createApp } from 'vue';
import App from './App.vue';
import router from './router';

// Seuls les composants réellement utilisés sont enregistrés globalement (les
// templates en écrivent certains en kebab-case : <b-row>, <b-col>…). Enregistrer
// toute la bibliothèque l'embarquait entière dans le bundle.
import {
    BAlert, BCol, BContainer, BFormInput, BFormSelect,
    BModal, BNav, BNavItem, BRow, BSpinner,
} from 'bootstrap-vue-next';
import 'bootstrap/dist/css/bootstrap.css';
import 'bootstrap-vue-next/dist/bootstrap-vue-next.css';
import '@fortawesome/fontawesome-free/css/all.css';
import './assets/fonts.css';
import './style.css';
import './assets/arcade-theme.css';

const app = createApp(App);

const components = {
    BAlert, BCol, BContainer, BFormInput, BFormSelect,
    BModal, BNav, BNavItem, BRow, BSpinner,
};
for (const [name, component] of Object.entries(components)) {
    app.component(name, component);
}

app.use(router);

app.mount('#app');
