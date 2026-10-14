import { useId } from "react";
import { logoSvg } from "./logoSvg";

/** The Unsealed logotype (see `logoSvg.ts`), at the width of its container. */
export function Logo({ className }: { className?: string }) {
  const id = useId().replace(/:/g, "");
  return <div className={`logo ${className ?? ""}`} dangerouslySetInnerHTML={{ __html: logoSvg(id) }} />;
}
