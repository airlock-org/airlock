"use client";

import type { ReactNode } from "react";
import { RootProvider } from "fumadocs-ui/provider/next";

export function Providers({ children }: { children: ReactNode }) {
  return (
    <RootProvider search={{ options: { type: "static", api: "/docs/search.json" } }}>
      {children}
    </RootProvider>
  );
}
