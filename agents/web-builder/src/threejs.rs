use crate::interpreter::ThreeDSpec;

pub fn generate_3d_scene(spec: &ThreeDSpec) -> String {
    let component_name = scene_component_name(spec.model.as_str());
    format!(
        "import {{ Canvas, useFrame }} from '@react-three/fiber';\n\
import {{ Suspense, useRef }} from 'react';\n\
import type {{ Mesh }} from 'three';\n\
\n\
function SceneMesh() {{\n\
  const mesh = useRef<Mesh | null>(null);\n\
  useFrame((_, delta) => {{\n\
    if (!mesh.current) return;\n\
    mesh.current.rotation.y += delta * 0.8;\n\
    mesh.current.rotation.x += delta * 0.2;\n\
  }});\n\
\n\
  return (\n\
    <mesh ref={{mesh}} position={{[0, 0, 0]}} castShadow>\n\
      <boxGeometry args={{[1.2, 1.2, 1.2]}} />\n\
      <meshStandardMaterial color=\"#00F5D4\" metalness={{0.2}} roughness={{0.35}} />\n\
    </mesh>\n\
  );\n\
}}\n\
\n\
export default function {component_name}() {{\n\
  return (\n\
    <div className=\"h-[420px] w-full rounded-3xl border border-cyan-300/40 bg-black/30\" aria-label=\"three scene\">\n\
      <Canvas camera={{{{ position: [0, 0.5, 3], fov: 50 }}}}>\n\
        <ambientLight intensity={{0.6}} />\n\
        <directionalLight position={{[2, 2, 3]}} intensity={{1.2}} />\n\
        <Suspense fallback={{null}}>\n\
          <SceneMesh />\n\
        </Suspense>\n\
      </Canvas>\n\
    </div>\n\
  );\n\
}}\n\
\n\
// model={model}, animation={animation}, position={position}\n",
        component_name = component_name,
        model = spec.model,
        animation = spec.animation,
        position = spec.position,
    )
}

pub fn scene_component_name(model: &str) -> String {
    let mut name = String::new();
    let mut upper_next = true;
    for ch in model.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                name.push(ch.to_ascii_uppercase());
                upper_next = false;
            } else {
                name.push(ch.to_ascii_lowercase());
            }
        } else {
            upper_next = true;
        }
    }
    if name.is_empty() {
        return "GeneratedScene".to_string();
    }
    format!("{}Scene", name)
}
