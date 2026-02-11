import { useEffect, useRef, useState } from "react";

export const useWindowWidth = () => {
  const [width, setWidth] = useState(() => document.body.clientWidth);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const handleResize = () => {
      if (timerRef.current) return;
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        setWidth(document.body.clientWidth);
      }, 150);
    };

    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

  return { width };
};
