import React from 'react';

export default function HomePage(): JSX.Element {
return (
<div className="space-y-8" data-layout="landing">
<nav className="sticky top-0 flex items-center justify-between bg-black/50 p-4" aria-label="main navigation">Brand</nav>
      <section className="py-24 text-center"><h1 className="text-6xl font-display">Hero Section</h1><p className="mx-auto mt-6 max-w-2xl">Build the website</p></section>
      <section className="grid gap-6 md:grid-cols-3"><article className="rounded-2xl border p-6">Feature A</article><article className="rounded-2xl border p-6">Feature B</article><article className="rounded-2xl border p-6">Feature C</article></section>
      <section className="grid gap-8 md:grid-cols-2"><form aria-label="contact form">Form</form><aside>Details</aside></section>
      <footer className="space-y-4 border-t border-white/20 py-10"><p>Footer links and social proof</p><form aria-label="newsletter">Subscribe</form></footer>
</div>
);
}
