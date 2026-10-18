import type { ReactNode } from "react";
import { motion } from "motion/react";
import { useLocation } from "react-router-dom";

const EASE = [0.21, 0.47, 0.32, 0.98] as const;

/** Route-level fade/rise — 200ms, state-only, no page theatrics. */
export function PageTransition({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  return (
    <motion.div
      key={pathname}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.22, ease: EASE }}
    >
      {children}
    </motion.div>
  );
}

/** Staggered children container for KPI/card grids. */
export function StaggerGrid({
  children,
  className,
  delayStep = 0.04,
}: {
  children: ReactNode;
  className?: string;
  delayStep?: number;
}) {
  return (
    <motion.div
      className={className}
      initial="hidden"
      animate="show"
      variants={{
        hidden: {},
        show: { transition: { staggerChildren: delayStep } },
      }}
    >
      {children}
    </motion.div>
  );
}
