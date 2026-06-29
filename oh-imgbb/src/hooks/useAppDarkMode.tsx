import { useEffect } from "react";
import { useAppStore } from "../tools/store";

const COLOR_SCHEME_QUERY = "(prefers-color-scheme: dark)";

export function useAppDarkMode() {
  const setState = useAppStore((s) => s.setState);

  useEffect(() => {
    const media = window.matchMedia(COLOR_SCHEME_QUERY);

    setState({ darkMode: media.matches });

    const handler = (e: MediaQueryListEvent) => {
      setState({ darkMode: e.matches });
    };

    media.addEventListener("change", handler);

    return () => media.removeEventListener("change", handler);
  }, [setState]);
}