struct Params { a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, d: vec4<f32> }
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
@group(0) @binding(2) var<uniform> p: Params;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vertex(@builtin(vertex_index) i:u32)->Vertex {
    var positions=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    let v=positions[i]; var o:Vertex; o.position=vec4(v,0.,1.); o.uv=vec2((v.x+1.)*.5,(1.-v.y)*.5); return o;
}
@fragment fn premultiply(v:Vertex)->@location(0) vec4<f32> {
    let c=textureSample(source,filtering,v.uv); return vec4(c.rgb*c.a,c.a);
}
@fragment fn color(v:Vertex)->@location(0) vec4<f32> {
    let c=textureSample(source,filtering,v.uv); if c.a<=0. {return vec4(0.);}
    let rgb=c.rgb/c.a; let l=dot(rgb,vec3(.2126,.7152,.0722));
    return vec4(clamp((vec3(l)+(rgb-vec3(l))*p.a.z-vec3(.5))*p.a.y+vec3(.5+p.a.x),vec3(0.),vec3(1.))*c.a,c.a);
}
// Pair consecutive Gaussian taps into one bilinear lookup. Radius is bounded by the validated graph.
@fragment fn blur(v:Vertex)->@location(0) vec4<f32> {
    let radius=i32(p.a.z); if radius==0 {return textureSample(source,filtering,v.uv);}
    let sigma=max(p.a.z/3.,.5); let denom=2.*sigma*sigma;
    var sum=textureSample(source,filtering,v.uv); var total=1.;
    for(var i=1;i<=radius;i+=2) {
        let w1=exp(-f32(i*i)/denom); var w2=0.;
        if i+1<=radius {w2=exp(-f32((i+1)*(i+1))/denom);}
        let weight=w1+w2; let offset=f32(i)+w2/weight;
        sum+=(textureSample(source,filtering,v.uv+p.a.xy*offset)+textureSample(source,filtering,v.uv-p.a.xy*offset))*weight;
        total+=2.*weight;
    }
    return sum/total;
}
@fragment fn place(v:Vertex)->@location(0) vec4<f32> {
    let xy=v.position.xy-p.b.xy;
    let q=vec2(dot(p.a.xy,xy),dot(p.a.zw,xy));
    let size=vec2<f32>(textureDimensions(source));
    // Explicit transparent-border bilinear sampling, matching the CPU path.
    let pos=q-vec2(.5); let base=vec2<i32>(floor(pos)); let f=fract(pos); var out=vec4(0.);
    for(var y=0;y<2;y++) {for(var x=0;x<2;x++) {
        let coord=base+vec2(x,y);
        if all(coord>=vec2(0)) && all(coord<vec2<i32>(size)) {
            let w=select(1.-f.x,f.x,x==1)*select(1.-f.y,f.y,y==1);
            out+=textureLoad(source,coord,0)*w;
        }
    }}
    return out*p.b.z;
}
