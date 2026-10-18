import { Nav } from './sections/Nav'
import { Hero } from './sections/Hero'
import { Terminal } from './sections/Terminal'
import { Race } from './sections/Race'
import { Playground } from './sections/Playground'
import { Tools } from './sections/Tools'
import { Benchmarks } from './sections/Benchmarks'
import { Start, Footer } from './sections/Start'

export default function App() {
  return (
    <>
      <Nav />
      <main>
        <Hero />
        <Terminal />
        <Tools />
        <Race />
        <Playground />
        <Benchmarks />
        <Start />
      </main>
      <Footer />
    </>
  )
}
