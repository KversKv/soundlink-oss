import type { Content } from './content/zh'
import { Nav } from './sections/Nav'
import { Hero } from './sections/Hero'
import { PlatformMatrix } from './sections/PlatformMatrix'
import { Scenario } from './sections/Scenario'
import { Differentiators } from './sections/Differentiators'
import { QuickResolution } from './sections/QuickResolution'
import { HowItWorks } from './sections/HowItWorks'
import { Specs } from './sections/Specs'
import { Limitations } from './sections/Limitations'
import { Download } from './sections/Download'
import { Footer } from './sections/Footer'

export function App({ c }: { c: Content }) {
  return (
    <>
      <Nav c={c} />
      <main>
        <Hero c={c} />
        <QuickResolution c={c} />
        <PlatformMatrix c={c} />
        <Scenario c={c} />
        <Differentiators c={c} />
        <HowItWorks c={c} />
        <Specs c={c} />
        <Limitations c={c} />
        <Download c={c} />
      </main>
      <Footer c={c} />
    </>
  )
}
