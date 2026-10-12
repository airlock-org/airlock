import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Providers } from "@/components/providers";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: "AirlockOS — Qubes architecture, declared in Nix", template: "%s | Airlock" },
  description: "An experimental Nix-native Qubes system with declarative host and guest systems, whole-machine deployment, and recorded verification.",
  icons: { icon: "/logo.svg" },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
