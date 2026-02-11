import { defineConfig } from "vite";

export default defineConfig(({ mode }) => {
  const isWeb = mode === "web";

  return {
    root: "src",
    base: isWeb ? "/PanEx/demo/" : "/",
    define: {
      __DISABLE_PAYWALL__: JSON.stringify(!!process.env.DISABLE_PAYWALL),
    },
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
