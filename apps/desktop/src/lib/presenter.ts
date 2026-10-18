/** Binary RGBA -> one texture upload. The ArrayBuffer is viewed without a second JS pixel copy. */
export function presenter(canvas: HTMLCanvasElement) {
  const gl = canvas.getContext("webgl2", {
    alpha: false,
    antialias: false,
    depth: false,
    preserveDrawingBuffer: false,
  });
  if (!gl) throw new Error("WebGL2 indisponível para a Prévia.");
  const shader = (type: number, source: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, source);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS))
      throw new Error(gl.getShaderInfoLog(s) ?? "Shader inválido");
    return s;
  };
  const vs = shader(
    gl.VERTEX_SHADER,
    `#version 300 es
  out vec2 uv;void main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);uv=vec2(p.x,1.0-p.y);gl_Position=vec4(p*2.0-1.0,0,1);}`,
  );
  const fs = shader(
    gl.FRAGMENT_SHADER,
    `#version 300 es
  precision mediump float;in vec2 uv;uniform sampler2D image;out vec4 color;void main(){color=texture(image,uv);}`,
  );
  const program = gl.createProgram()!;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS))
    throw new Error("Falha ao vincular shaders da Prévia.");
  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  let width = 0,
    height = 0,
    hasFrame = false;
  const draw = () => {
    const w = Math.max(1, Math.round(canvas.clientWidth * devicePixelRatio)),
      h = Math.max(1, Math.round(canvas.clientHeight * devicePixelRatio));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!hasFrame) return;
    const scale = Math.min(w / width, h / height),
      dw = Math.round(width * scale),
      dh = Math.round(height * scale);
    gl.viewport(Math.floor((w - dw) / 2), Math.floor((h - dh) / 2), dw, dh);
    gl.useProgram(program);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };
  return {
    draw,
    clear() {
      hasFrame = false;
      draw();
    },
    upload(bytes: ArrayBuffer, w: number, h: number) {
      if (
        w < 1 ||
        h < 1 ||
        w > 1920 ||
        h > 1080 ||
        bytes.byteLength !== w * h * 4
      )
        throw new Error("Frame binário inválido.");
      gl.bindTexture(gl.TEXTURE_2D, texture);
      if (width !== w || height !== h) {
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.RGBA,
          w,
          h,
          0,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          new Uint8Array(bytes),
        );
      } else {
        gl.texSubImage2D(
          gl.TEXTURE_2D,
          0,
          0,
          0,
          w,
          h,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          new Uint8Array(bytes),
        );
      }
      width = w;
      height = h;
      hasFrame = true;
      draw();
    },
    dispose() {
      gl.deleteTexture(texture);
      gl.deleteProgram(program);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
    },
  };
}
