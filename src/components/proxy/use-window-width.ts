import { useEffect, useState } from "react";

export const useWindowWidth = () => {
  const [width, setWidth] = useState(() => document.body.clientWidth);

  useEffect(() => {
    const handleResize = () => setWidth(document.body.clientWidth);

    window.addEventListener("resize", handleResize);
    // 兜底：WebView2 从托盘恢复初期 window.resize 可能漏触发，
    // 用 ResizeObserver 监听 body，确保列宽计算用到正确的初始 width
    const observer = new ResizeObserver(() => {
      setWidth(document.body.clientWidth);
    });
    observer.observe(document.body);
    return () => {
      window.removeEventListener("resize", handleResize);
      observer.disconnect();
    };
  }, []);

  return { width };
};
