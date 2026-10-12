import type { ReactNode } from "react";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import { BrandLabel } from "@/components/brand";
import { source } from "@/lib/source";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout
      nav={{ title: <BrandLabel />, url: "/" }}
      tree={source.getPageTree()}
      sidebar={{ defaultOpenLevel: 1 }}
      links={[
        { text: "Home", url: "/" },
        { text: "GitHub", url: "https://github.com/airlock-org/airlock", external: true },
      ]}
    >
      {children}
    </DocsLayout>
  );
}
