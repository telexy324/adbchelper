import { PropsWithChildren } from "react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";

interface SectionCardProps extends PropsWithChildren {
  eyebrow: string;
  title: string;
  description: string;
}

export function SectionCard({
  eyebrow,
  title,
  description,
  children,
}: SectionCardProps) {
  return (
    <Card className="border-white/45 bg-card/82 shadow-[0_24px_90px_-42px_rgba(14,116,144,0.45)] backdrop-blur-xl">
      <CardHeader className="space-y-3">
        <p className="text-xs font-medium uppercase tracking-[0.24em] text-muted-foreground">{eyebrow}</p>
        <CardTitle className="font-serif text-2xl">{title}</CardTitle>
        <CardDescription className="max-w-2xl text-sm leading-6">{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">{children}</CardContent>
    </Card>
  );
}
