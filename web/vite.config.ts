import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import preprocess from "svelte-preprocess";
import { mdsvex } from "mdsvex";



// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
    const baseUrl = mode === "development" ? "http://localhost:8000" : "";

    return {
        plugins: [svelte({
            extensions: [
                '.svelte', '.svx', '.md'
            ],
            preprocess: [
                mdsvex({ extensions: ['svx', '.md'] }),
                preprocess(),
            ],
        })],
        define: {
            BASE_URL: JSON.stringify(baseUrl)
        }
    }
})