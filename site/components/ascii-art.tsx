"use client";

import { useEffect, useState } from "react";

export function AsciiLogo() {
  const [isShimmering, setIsShimmering] = useState(false);

  useEffect(() => {
    // Initial shimmer after mount
    const initialTimeout = setTimeout(() => {
      setIsShimmering(true);
    }, 500);

    // Set up periodic shimmer every 4 seconds
    const interval = setInterval(() => {
      setIsShimmering(true);
      // Reset after animation completes
      setTimeout(() => setIsShimmering(false), 1500);
    }, 4000);

    return () => {
      clearTimeout(initialTimeout);
      clearInterval(interval);
    };
  }, []);

  return (
    <div className="relative inline-block">
      {/* Base logo layer */}
      <pre className="font-mono text-xs leading-tight text-primary sm:text-sm md:text-base" style={{ lineHeight: "1.2rem" }}>
        {`███████╗██████╗  █████╗  █████╗ 
██╔════╝██╔══██╗██╔══██╗██╔══██╗
███████╗██████╔╝███████║███████║
╚════██║██╔═══╝ ██╔══██║██╔══██║
███████║██║     ██║  ██║██║  ██║
╚══════╝╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝`}
      </pre>

      {/* Shimmer overlay with mask */}
      <pre
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 font-mono text-xs leading-tight sm:text-sm md:text-base"
        style={{
          background:
            "linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.8) 50%, transparent 100%)",
          backgroundSize: "200% 100%",
          WebkitBackgroundClip: "text",
          backgroundClip: "text",
          color: "transparent",
          animation: isShimmering ? "shimmer 1.5s ease-in-out" : "none",
          mixBlendMode: "overlay",
          lineHeight: "1.2rem",
        }}
      >
        {`███████╗██████╗  █████╗  █████╗ 
██╔════╝██╔══██╗██╔══██╗██╔══██╗
███████╗██████╔╝███████║███████║
╚════██║██╔═══╝ ██╔══██║██╔══██║
███████║██║     ██║  ██║██║  ██║
╚══════╝╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝`}
      </pre>

      {/* CSS animation */}
      <style jsx>{`
        @keyframes shimmer {
          0% {
            background-position: 200% 0;
          }
          100% {
            background-position: -200% 0;
          }
        }
      `}</style>
    </div>
  );
}
