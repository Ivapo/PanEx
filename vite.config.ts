import { defineConfig } from "vite";

export default defineConfig(({ mode }) => {
  const isWeb = mode === "web";

  return {
    root: "src",
    build: {
      outDir: isWeb ? "../dist-web" : "../dist",
      emptyOutDir: true,
      rollupOptions: isWeb
        ? {
            external: ["@tauri-apps/api/core"],
          }
        : {},
    },
    server: {
      port: 1420,
      strictPort: true,
    },
  };
});
