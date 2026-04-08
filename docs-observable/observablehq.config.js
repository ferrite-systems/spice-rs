export default {
  title: "spice-rs",
  root: "src",
  theme: "light",
  search: true,
  toc: true,
  pager: true,
  pages: [
    {name: "Introduction", path: "/"},
    {
      name: "Learn SPICE",
      open: true,
      pages: [
        {name: "1. What Is Circuit Simulation?", path: "/learn/ch01"},
        {name: "2. Modified Nodal Analysis", path: "/learn/ch02"},
        {name: "3. DC Operating Point", path: "/learn/ch03"},
        {name: "4. The Diode", path: "/learn/ch04"},
        {name: "5. The MOSFET", path: "/learn/ch05"},
        {name: "6. The BJT", path: "/learn/ch06"},
        {name: "7. The JFET", path: "/learn/ch07"},
        {name: "8. AC Analysis", path: "/learn/ch08"},
        {name: "9. Transient Analysis", path: "/learn/ch09"},
        {name: "10. Sources & Waveforms", path: "/learn/ch10"},
        {name: "11. Reactive Elements", path: "/learn/ch11"},
        {name: "12. Advanced Analysis", path: "/learn/ch12"},
      ]
    },
    {
      name: "Reference",
      pages: [
        {name: "13. Netlist Syntax", path: "/reference/ch13"},
        {name: "14. Device Models", path: "/reference/ch14"},
        {name: "15. Simulation Options", path: "/reference/ch15"},
        {name: "16. API Reference", path: "/reference/ch16"},
      ]
    },
    {
      name: "Internals",
      pages: [
        {name: "17. Architecture", path: "/internals/ch17"},
        {name: "18. Porting Process", path: "/internals/ch18"},
        {name: "19. sparse-rs", path: "/internals/ch19"},
        {name: "20. Validation", path: "/internals/ch20"},
      ]
    },
    {name: "Licensing & Attribution", path: "/license"},
  ],
  head: `<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Crimson+Pro:ital,wght@0,400;0,600;0,700;1,400&display=swap" rel="stylesheet">
<style>
:root {
  --theme-foreground: #3b2f20;
  --theme-background: #faf4e8;
  --theme-background-alt: #f0e8d4;
  --theme-foreground-alt: #6b5d4d;
  --theme-foreground-muted: #9e9788;
  --theme-accent: #b87333;
  --theme-blue: #4a6fa5;
}
body, .observablehq {
  font-family: 'Crimson Pro', Georgia, serif;
  font-size: 19px;
  line-height: 1.65;
}
h1, h2, h3 { font-family: 'Crimson Pro', Georgia, serif; }
h1 { border-bottom: 2px solid var(--theme-accent); padding-bottom: 0.2em; }
code, pre code { font-family: 'Iosevka', 'Fira Code', monospace; }
a { color: var(--theme-accent); }
.ferrite-circuit { text-align: center; margin: 1em 0; }
.ferrite-circuit svg { max-width: 100%; height: auto; display: inline-block; }
</style>`
};
