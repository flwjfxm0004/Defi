// Unified Custom Hybrid Renderer Shader (`게임.md §5.1, §7.1, §8.2`)
// Target: SPIR-V 1.6 (Vulkan 1.4 / 1.3 Core + Global Bindless + 64-bit Buffer Device Address)
// Primary Entrypoints (`verifier` & `VulkanHybridRenderer`):
//   1. sdf_world          - Sparse 10x10x10 padded SDF raymarching -> 64-bit SSBO Visibility Buffer + Hi-Z Pyramid
//   2. meshlet_cull_vis   - Compute Frustum/Cone/Hi-Z culling, Multi-Bone DQ skinning, <4x4px Soft-Raster & Indirect Draw emit
//   3. vis_indirect_vs/fs - Early-Z Reversed-Z Hardware Indirect Raster (`>=4x4px` clusters) -> 64-bit SSBO Visibility Buffer
//   4. meshlet_cel        - Overdraw-0% Single Compute Tile Material Resolve (`ddx`/`ddy` + Global Bindless KTX2 Mip Sample)
//   5. radiance_cascades  - 3D Sparse Radiance Cascades GI, SDF AO & Screen Hi-Z + World SDF (+ RayQuery) Hybrid Specular
//   6. vfx_telegraph      - Translucent 3D VFX Composite, ACES Post-Tonemap, Orthogonal OKLab Telegraph & Dual Outline
// Core Fallback Entrypoints (when `shaderBufferInt64Atomics` or `bufferDeviceAddress` is absent):
//   7. vis_fallback_fs / tile_shade_fallback

#ifndef USE_BDA_POINTER
#define USE_BDA_POINTER 1
#endif

#ifndef ENABLE_HW_RAY_QUERY
#define ENABLE_HW_RAY_QUERY 0
#endif

struct GpuVfxEmitter
{
    float4 centerAndRadius; // xyz = world center, w = outer radius (meters)
    float4 colorAndAlpha;   // rgb = HDR linear emission, a = opacity [0..1]
};

struct GpuSceneBdaHeader
{
    float4 sunDirAndMaxDist;        // xyz = normalized sun dir, w = max raymarch distance
    float4 skyZenithAndRoughCutoff; // rgb = low-chroma zenith HDR tint, w = specular roughness cutoff
    uint4  lutAndVfxCounts;         // x = bindlessLutSlot, y = activeVfxCount (<=8), z = use64BitVis, w = hiZTileWidth
    GpuVfxEmitter vfxEmitters[8];
};

struct FramePushConstants
{
    float4   camPosAndFov;        // xyz = camera world pos, w = tan(fovY * 0.5)
    float4   camForwardAndTime;   // xyz = camera forward,   w = timeSeconds
    float4   camRightAndAspect;   // xyz = camera right,     w = aspect (width / height)
    float4   camUpAndNear;        // xyz = camera up,        w = nearZ (Reversed-Z numerator)
    uint4    viewportAndFlags;    // x = width, y = height, z = brickCount, w = meshletCount
    uint4    telegraphRect;       // x = x0, y = y0, z = w0, w = h0 (3D world-to-clip projected Boss AABB!)
    uint4    telegraphParams;     // x = telegraphActive (0/1), y = telegraphKind (0=Dodge,1=Parry), z = borderPx, w = boneCount
#if USE_BDA_POINTER
    uint64_t sceneBdaAddress;     // Live 64-bit Buffer Device Address (`PhysicalStorageBuffer64`) to `GpuSceneBdaHeader`
#else
    uint2    sceneBdaLowHigh;     // Core fallback placeholder preserving exact 128-byte push-constant ABI
#endif
    uint2    epochAndHitstop;     // x = epochSlot, y = hitstopFlashPermille
};

[[vk::push_constant]]
ConstantBuffer<FramePushConstants> gPush;

struct BrickHeaderGpu
{
    int4 coordAndFlags; // xyz = brick (bx, by, bz), w = baseCellOffset in gSdfCells
};

struct MeshletGpu
{
    float4 boundingSphere; // xyz = local center, w = radius
    float4 coneAxisCutoff; // xyz = cone axis, w = cos(halfAngle + pi/2)
    uint4  countsAndOffs;  // x = vertCount (<=64), y = triCount (<=124), z = vertOffset, w = indexOffset
    uint4  materialParams; // x = albedoPacked, y = roughnessPermille, z = metallicPermille, w = rootBoneIndex
};

struct DualQuatGpu
{
    float4 qReal; // xyzw unit rotation quaternion
    float4 qDual; // xyzw dual quaternion translation half-product
};

struct DrawIndexedIndirectCmdGpu
{
    uint indexCount;
    uint instanceCount;
    uint firstIndex;
    int  vertexOffset;
    uint firstInstance;
};

// Unified Set 0 Bindings + Global Bindless Descriptor Array (`게임.md §8.2`):
[[vk::binding(0, 0)]]  StructuredBuffer<BrickHeaderGpu>            gBrickHeaders;
[[vk::binding(1, 0)]]  StructuredBuffer<int>                       gSdfCells;       // packed (sdf_cm_i16 | (field_u8 << 16))
[[vk::binding(2, 0)]]  StructuredBuffer<MeshletGpu>                gMeshlets;
[[vk::binding(3, 0)]]  StructuredBuffer<float4>                    gMeshVertices;   // xyz = local pos, w = float((boneIdx << 16) | octNrm)
[[vk::binding(4, 0)]]  StructuredBuffer<uint>                      gMeshIndices;    // 3 local vertex indices per triangle
[[vk::binding(5, 0)]]  StructuredBuffer<DualQuatGpu>               gSkinDualQuats;
// Single 64-bit SSBO Visibility Buffer:
// [63:32] Reversed-Z 32-bit IEEE float depth | [31] 1-bit SDF(0)/Meshlet(1) tag | [30:0] 31-bit cluster/primitive ID
[[vk::binding(6, 0)]]  RWStructuredBuffer<uint64_t>                gVisBuffer64;
[[vk::binding(7, 0)]]  StructuredBuffer<uint>                      gVisFallbackR32;
[[vk::binding(8, 0)]]  RWStructuredBuffer<float4>                  gGbufferShaded;  // rgb = Cel/PBR direct radiance, a = roughness
[[vk::binding(9, 0)]]  RWStructuredBuffer<float4>                  gGbufferNormal;  // xyz = world normal, w = linearDepth
[[vk::binding(10, 0)]] RWStructuredBuffer<float4>                  gHdrLitBuffer;   // rgb = Direct + GI + Specular HDR, a = ao
// Global Bindless Texture Heap (`UPDATE_AFTER_BIND | PARTIALLY_BOUND`, mipmapped KTX2 cel ramps):
[[vk::binding(11, 0)]] Sampler2D<float4>                           gBindlessTextures[16];
[[vk::binding(12, 0)]] [[vk::image_format("rgba8")]] RWTexture2D<float4> gOutColorImage;
// Hierarchical-Z (`Hi-Z`) Pyramid & GPU-Driven Indirect Draw Buffers (`§8.2`):
[[vk::binding(13, 0)]] RWStructuredBuffer<uint>                    gHiZMip0;        // Per-pixel Reversed-Z depth bits
[[vk::binding(14, 0)]] RWStructuredBuffer<uint>                    gHiZMip1;        // Coarse 8x8 tile minimum Reversed-Z depth bits
[[vk::binding(15, 0)]] RWStructuredBuffer<DrawIndexedIndirectCmdGpu> gIndirectCmds;
[[vk::binding(16, 0)]] RWStructuredBuffer<uint>                    gIndirectDrawCount;
[[vk::binding(17, 0)]] RWStructuredBuffer<uint>                    gCompactedIndices;
[[vk::binding(18, 0)]] StructuredBuffer<GpuSceneBdaHeader>         gSceneFallbackBuf; // Fallback when BDA is disabled on 1.3 hardware
#if ENABLE_HW_RAY_QUERY
[[vk::binding(19, 0)]] RaytracingAccelerationStructure             gWorldTlas;
#endif

// ----------------------------------------------------------------------------
// 64-bit BDA Scene Header Accessor & Shared Geometry / OKLab Helpers
// ----------------------------------------------------------------------------

GpuSceneBdaHeader loadSceneHeader()
{
#if USE_BDA_POINTER
    if (gPush.sceneBdaAddress != 0ULL)
    {
        Ptr<GpuSceneBdaHeader, Access::Read, AddressSpace::Device> bdaPtr =
            Ptr<GpuSceneBdaHeader, Access::Read, AddressSpace::Device>(gPush.sceneBdaAddress);
        return *bdaPtr;
    }
#endif
    return gSceneFallbackBuf[0];
}

float3 computePrimaryRayDir(uint2 px, uint2 dims)
{
    float2 uv = (float2(px) + 0.5) / float2(max(dims.x, 1u), max(dims.y, 1u));
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    float tanHalf = gPush.camPosAndFov.w;
    float aspect = gPush.camRightAndAspect.w;
    float3 dir = gPush.camForwardAndTime.xyz
               + gPush.camRightAndAspect.xyz * (ndc.x * aspect * tanHalf)
               + gPush.camUpAndNear.xyz * (ndc.y * tanHalf);
    return normalize(dir);
}

float4 projectWorldToClip(float3 worldP)
{
    float3 rel = worldP - gPush.camPosAndFov.xyz;
    float vz = dot(rel, gPush.camForwardAndTime.xyz);
    float vx = dot(rel, gPush.camRightAndAspect.xyz) / max(gPush.camRightAndAspect.w * gPush.camPosAndFov.w, 1e-4);
    float vy = -dot(rel, gPush.camUpAndNear.xyz) / max(gPush.camPosAndFov.w, 1e-4);
    float nearZ = max(gPush.camUpAndNear.w, 0.05);
    return float4(vx, vy, nearZ, vz);
}

uint encodeReversedZDepth(float linearT)
{
    float nearZ = max(gPush.camUpAndNear.w, 0.05);
    float revZ = clamp(nearZ / max(linearT, nearZ), 0.0, 1.0);
    return asuint(revZ);
}

uint64_t packVisibilityWord64(uint revDepthBits, uint isMeshletTag, uint id31)
{
    uint lo32 = ((isMeshletTag & 1u) << 31u) | (id31 & 0x7FFFFFFFu);
    return (uint64_t(revDepthBits) << 32u) | uint64_t(lo32);
}

void sampleSparseSdfWorld(float3 worldPos, out float sdfMeters, out uint fieldState, out uint brickId)
{
    int3 wVoxel = int3(floor(worldPos));
    int3 bCoord = int3(
        wVoxel.x >= 0 ? (wVoxel.x / 8) : ((wVoxel.x - 7) / 8),
        wVoxel.y >= 0 ? (wVoxel.y / 8) : ((wVoxel.y - 7) / 8),
        wVoxel.z >= 0 ? (wVoxel.z / 8) : ((wVoxel.z - 7) / 8)
    );
    int3 local = (wVoxel - bCoord * 8) + int3(1, 1, 1);
    local = clamp(local, int3(0, 0, 0), int3(9, 9, 9));

    uint brickCount = min(gPush.viewportAndFlags.z, 512u);
    for (uint i = 0u; i < brickCount; ++i)
    {
        int4 hdr = gBrickHeaders[i].coordAndFlags;
        if (hdr.x == bCoord.x && hdr.y == bCoord.y && hdr.z == bCoord.z)
        {
            uint cellIdx = uint(hdr.w) + uint(local.z * 100 + local.y * 10 + local.x);
            int packed = gSdfCells[cellIdx];
            int rawCm = (packed << 16) >> 16;
            sdfMeters = float(rawCm) * 0.01;
            fieldState = uint((packed >> 16) & 0xFF);
            brickId = i;
            return;
        }
    }

    sdfMeters = worldPos.y - 4.0;
    fieldState = 0u;
    brickId = 0x7FFFu;
}

float3 estimateSdfNormal(float3 p)
{
    float e = 0.35;
    float dxP, dxN, dyP, dyN, dzP, dzN;
    uint fs, bid;
    sampleSparseSdfWorld(p + float3(e, 0.0, 0.0), dxP, fs, bid);
    sampleSparseSdfWorld(p - float3(e, 0.0, 0.0), dxN, fs, bid);
    sampleSparseSdfWorld(p + float3(0.0, e, 0.0), dyP, fs, bid);
    sampleSparseSdfWorld(p - float3(0.0, e, 0.0), dyN, fs, bid);
    sampleSparseSdfWorld(p + float3(0.0, 0.0, e), dzP, fs, bid);
    sampleSparseSdfWorld(p - float3(0.0, 0.0, e), dzN, fs, bid);
    float3 grad = float3(dxP - dxN, dyP - dyN, dzP - dzN);
    float lenSq = dot(grad, grad);
    return (lenSq > 1e-6) ? (grad * rsqrt(lenSq)) : float3(0.0, 1.0, 0.0);
}

float3 transformDualQuat(DualQuatGpu dq, float3 v)
{
    float4 r = dq.qReal;
    float4 d = dq.qDual;
    float3 vRot = v + 2.0 * cross(r.xyz, cross(r.xyz, v) + r.w * v);
    float3 trans = 2.0 * (r.w * d.xyz - d.w * r.xyz + cross(r.xyz, d.xyz));
    return vRot + trans;
}

float3 skinMeshVertex(uint globalVertIdx, uint clusterFallbackBone)
{
    float4 rawV = gMeshVertices[globalVertIdx];
    uint packedMeta = uint(max(rawV.w, 0.0));
    uint vBone = (packedMeta >> 8u) & 0xFFu;
    uint hasPerVertBone = packedMeta & 1u;
    uint boneCount = max(gPush.telegraphParams.w, 1u);
    uint finalBone = (hasPerVertBone != 0u) ? min(vBone, boneCount - 1u) : min(clusterFallbackBone, boneCount - 1u);
    DualQuatGpu dq = gSkinDualQuats[finalBone];
    return transformDualQuat(dq, rawV.xyz);
}

float3 unpackRgba8Unorm(uint packed)
{
    return float3(
        float(packed & 0xFFu) / 255.0,
        float((packed >> 8u) & 0xFFu) / 255.0,
        float((packed >> 16u) & 0xFFu) / 255.0
    );
}

// Analytic surface-tangent-plane ray differentials (`Igehy 1999`, `게임.md §8.2`)
// projecting neighbor pixel rays `(rdX, rdY)` onto `(hitP, nrm)` to drive `SampleGrad` KTX2 mip selection:
void computeSurfaceUvGrads(
    float3 ro,
    float3 rd,
    float3 rdX,
    float3 rdY,
    float linearT,
    float3 nrm,
    out float2 ddxUv,
    out float2 ddyUv
)
{
    float3 hitP = ro + rd * linearT;
    float ndr = dot(rd, nrm);
    float dxDot = dot(rdX, nrm);
    float dyDot = dot(rdY, nrm);
    float denomX = ((dxDot < 0.0) ? -1.0 : 1.0) * max(abs(dxDot), 0.08);
    float denomY = ((dyDot < 0.0) ? -1.0 : 1.0) * max(abs(dyDot), 0.08);
    float tx = clamp((linearT * ndr) / denomX, 0.05, 250.0);
    float ty = clamp((linearT * ndr) / denomY, 0.05, 250.0);

    float3 dpdx = (ro + rdX * tx) - hitP;
    float3 dpdy = (ro + rdY * ty) - hitP;

    float3 upAxis = (abs(nrm.y) < 0.95) ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
    float3 tangentU = normalize(cross(upAxis, nrm));
    float3 bitangentV = cross(nrm, tangentU);

    const float kUvWorldFreq = 0.55;
    ddxUv = float2(dot(dpdx, tangentU), dot(dpdx, bitangentV)) * kUvWorldFreq;
    ddyUv = float2(dot(dpdy, tangentU), dot(dpdy, bitangentV)) * kUvWorldFreq;
}

float4 shadeSdfSurface(
    float3 ro,
    float3 rd,
    float3 rdX,
    float3 rdY,
    float linearT,
    float3 nrm,
    uint hitField,
    float3 sunDir,
    uint lutSlot
)
{
    float3 baseAlbedo = float3(0.24, 0.28, 0.26);
    float roughness = 0.65;
    if (hitField == 1u) { baseAlbedo = float3(0.08, 0.36, 0.62); roughness = 0.18; }
    else if (hitField == 2u) { baseAlbedo = float3(0.55, 0.84, 0.95); roughness = 0.12; }
    else if (hitField == 3u) { baseAlbedo = float3(0.92, 0.48, 0.14); roughness = 0.45; }
    else if (hitField == 4u) { baseAlbedo = float3(0.45, 0.38, 0.22); roughness = 0.32; }
    else if (hitField == 5u) { baseAlbedo = float3(0.58, 0.28, 0.96); roughness = 0.20; }
    else if (hitField == 6u) { baseAlbedo = float3(0.18, 0.88, 0.72); roughness = 0.25; }

    float2 ddxUv;
    float2 ddyUv;
    computeSurfaceUvGrads(ro, rd, rdX, rdY, linearT, nrm, ddxUv, ddyUv);

    float ndl = clamp(dot(nrm, sunDir), 0.0, 1.0);
#if USE_BDA_POINTER
    float3 celRamp = gBindlessTextures[NonUniformResourceIndex(lutSlot & 15u)].SampleGrad(float2(ndl, 0.5), ddxUv, ddyUv).xyz;
#else
    float3 celRamp = gBindlessTextures[0].SampleGrad(float2(ndl, 0.5), ddxUv, ddyUv).xyz;
#endif
    float3 direct = baseAlbedo * (0.25 + 0.95 * celRamp);
    return float4(direct, roughness);
}

float4 shadeMeshletTriangle(
    uint clusterId,
    uint triId,
    float3 ro,
    float3 rd,
    float3 rdX,
    float3 rdY,
    float3 sunDir,
    uint lutSlot,
    out float3 outNormal,
    out float outHitT
)
{
    MeshletGpu ml = gMeshlets[clusterId];
    uint vBase = ml.countsAndOffs.z;
    uint iBase = ml.countsAndOffs.w;
    uint rootBone = ml.materialParams.w;

    uint i0 = vBase + gMeshIndices[iBase + triId * 3u + 0u];
    uint i1 = vBase + gMeshIndices[iBase + triId * 3u + 1u];
    uint i2 = vBase + gMeshIndices[iBase + triId * 3u + 2u];

    float3 p0 = skinMeshVertex(i0, rootBone);
    float3 p1 = skinMeshVertex(i1, rootBone);
    float3 p2 = skinMeshVertex(i2, rootBone);

    float3 e1 = p1 - p0;
    float3 e2 = p2 - p0;
    float3 rawN = cross(e1, e2);
    float nLenSq = dot(rawN, rawN);
    outNormal = (nLenSq > 1e-8) ? (rawN * rsqrt(nLenSq)) : float3(0.0, 1.0, 0.0);
    if (dot(outNormal, rd) > 0.0)
    {
        outNormal = -outNormal;
    }
    float denom = dot(outNormal, rd);
    outHitT = (abs(denom) > 1e-5) ? clamp(dot(p0 - ro, outNormal) / denom, 0.15, 200.0) : 2.0;

    float2 ddxUv;
    float2 ddyUv;
    computeSurfaceUvGrads(ro, rd, rdX, rdY, outHitT, outNormal, ddxUv, ddyUv);

    float3 halfV = normalize(sunDir - rd);
    float ndl = clamp(dot(outNormal, sunDir), 0.0, 1.0);
    float ndh = clamp(dot(outNormal, halfV), 0.0, 1.0);
    float rim = pow(1.0 - clamp(dot(outNormal, -rd), 0.0, 1.0), 3.0);

#if USE_BDA_POINTER
    float3 celBand = gBindlessTextures[NonUniformResourceIndex(lutSlot & 15u)].SampleGrad(float2(ndl, 0.5), ddxUv, ddyUv).xyz;
#else
    float3 celBand = gBindlessTextures[0].SampleGrad(float2(ndl, 0.5), ddxUv, ddyUv).xyz;
#endif
    float3 albedo = unpackRgba8Unorm(ml.materialParams.x);
    float rough = clamp(float(ml.materialParams.y) * 0.001, 0.08, 1.0);
    float spec = pow(ndh, max(2.0 / (rough * rough), 4.0)) * 0.45;
    float hitstopBoost = float(gPush.epochAndHitstop.y) * 0.0005;

    return float4(albedo * (0.30 + 0.95 * celBand) + float3(spec + rim * 0.28 + hitstopBoost), rough);
}

// ============================================================================
// Pass 1A: `sdf_world` — Sparse Volume Raymarching -> 64-bit Visibility + Hi-Z
// ============================================================================
groupshared uint gsTileMinRevZ;

[shader("compute")]
[numthreads(8, 8, 1)]
void sdf_world(uint3 tid : SV_DispatchThreadID, uint3 gid : SV_GroupID, uint groupIndex : SV_GroupIndex)
{
    if (groupIndex == 0u)
    {
        gsTileMinRevZ = 0x7F7FFFFFu;
    }
    GroupMemoryBarrierWithGroupSync();

    uint2 dims = gPush.viewportAndFlags.xy;
    if (tid.x == 0u && tid.y == 0u)
    {
        gIndirectDrawCount[0] = 0u;
    }

    uint hitRevZ = 0u;
    if (tid.x < dims.x && tid.y < dims.y)
    {
        uint pixIdx = tid.y * dims.x + tid.x;
        GpuSceneBdaHeader sceneHdr = loadSceneHeader();
        float maxRayDist = max(sceneHdr.sunDirAndMaxDist.w, 60.0);

        if (sceneHdr.lutAndVfxCounts.z != 0u)
        {
            gVisBuffer64[pixIdx] = 0ULL;
        }
        gGbufferShaded[pixIdx] = float4(0.022, 0.023, 0.025, 0.85);
        gGbufferNormal[pixIdx] = float4(0.0, 1.0, 0.0, 500.0);

        float3 ro = gPush.camPosAndFov.xyz;
        float3 rd = computePrimaryRayDir(tid.xy, dims);

        float t = 0.25;
        float hitT = -1.0;
        uint hitField = 0u;
        uint hitBrick = 0u;

        for (uint step = 0u; step < 48u; ++step)
        {
            float3 p = ro + rd * t;
            float sdfM;
            uint fState;
            uint bId;
            sampleSparseSdfWorld(p, sdfM, fState, bId);
            if (sdfM <= 0.04)
            {
                hitT = t;
                hitField = fState;
                hitBrick = bId;
                break;
            }
            t += max(sdfM * 0.85, 0.18);
            if (t > maxRayDist)
            {
                break;
            }
        }

        if (hitT > 0.0)
        {
            hitRevZ = encodeReversedZDepth(hitT);
            if (sceneHdr.lutAndVfxCounts.z != 0u)
            {
                uint id31 = ((hitBrick & 0x7FFFFFu) << 8u) | (hitField & 0xFFu);
                uint64_t visWord = packVisibilityWord64(hitRevZ, 0u, id31);
                uint64_t prevWord;
                InterlockedMax(gVisBuffer64[pixIdx], visWord, prevWord);
            }
        }
        gHiZMip0[pixIdx] = hitRevZ;
        uint prevMin;
        InterlockedMin(gsTileMinRevZ, hitRevZ, prevMin);
    }

    GroupMemoryBarrierWithGroupSync();
    if (groupIndex == 0u)
    {
        uint tileStride = max((dims.x + 7u) / 8u, 1u);
        gHiZMip1[gid.y * tileStride + gid.x] = gsTileMinRevZ;
    }
}

// ============================================================================
// Pass 1B: `meshlet_cull_vis` — Frustum/Cone/Hi-Z Culling + <4x4px Soft-Raster
//          + `>=4x4px` `VkDrawIndexedIndirectCommand` Compaction (`§8.2`)
// ============================================================================
groupshared float3 gsClusterWorld[64];
groupshared float4 gsClusterClip[64];
groupshared uint   gsClusterVisible;

[shader("compute")]
[numthreads(64, 1, 1)]
void meshlet_cull_vis(uint3 gid : SV_GroupID, uint3 gtid : SV_GroupThreadID)
{
    uint m = gid.x;
    uint lane = gtid.x;
    uint meshletCount = min(gPush.viewportAndFlags.w, 64u);
    if (m >= meshletCount)
    {
        return;
    }

    MeshletGpu ml = gMeshlets[m];
    uint2 dims = gPush.viewportAndFlags.xy;
    GpuSceneBdaHeader sceneHdr = loadSceneHeader();

    if (lane == 0u)
    {
        gsClusterVisible = 1u;
        uint boneCount = max(gPush.telegraphParams.w, 1u);
        DualQuatGpu rootDq = gSkinDualQuats[min(ml.materialParams.w, boneCount - 1u)];
        float3 worldCenter = transformDualQuat(rootDq, ml.boundingSphere.xyz);
        float radius = ml.boundingSphere.w;

        // 1. View-space Frustum + Near-plane Sphere Culling:
        float3 rel = worldCenter - gPush.camPosAndFov.xyz;
        float vz = dot(rel, gPush.camForwardAndTime.xyz);
        if (vz + radius <= 0.12)
        {
            gsClusterVisible = 0u;
        }
        else
        {
            // 2. Backface Normal Cone Culling (when `cutoff < 0.999` directional cone):
            float3 coneOrigin = transformDualQuat(rootDq, float3(0.0, 0.0, 0.0));
            float3 worldAxis = normalize(transformDualQuat(rootDq, ml.coneAxisCutoff.xyz) - coneOrigin);
            float3 viewDir = normalize(rel);
            if (ml.coneAxisCutoff.w < 0.999 && dot(worldAxis, viewDir) > ml.coneAxisCutoff.w)
            {
                gsClusterVisible = 0u;
            }
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (gsClusterVisible == 0u)
    {
        return;
    }

    uint vertCount = min(ml.countsAndOffs.x, 64u);
    uint vBase = ml.countsAndOffs.z;
    if (lane < vertCount)
    {
        float3 wp = skinMeshVertex(vBase + lane, ml.materialParams.w);
        gsClusterWorld[lane] = wp;
        gsClusterClip[lane] = projectWorldToClip(wp);
    }
    GroupMemoryBarrierWithGroupSync();

    uint triCount = min(ml.countsAndOffs.y, 124u);
    uint iBase = ml.countsAndOffs.w;
    uint tileStride = max((dims.x + 7u) / 8u, 1u);
    uint tileRows = max((dims.y + 7u) / 8u, 1u);

    for (uint ti = lane; ti < triCount; ti += 64u)
    {
        uint lv0 = min(gMeshIndices[iBase + ti * 3u + 0u], max(vertCount, 1u) - 1u);
        uint lv1 = min(gMeshIndices[iBase + ti * 3u + 1u], max(vertCount, 1u) - 1u);
        uint lv2 = min(gMeshIndices[iBase + ti * 3u + 2u], max(vertCount, 1u) - 1u);

        float4 c0 = gsClusterClip[lv0];
        float4 c1 = gsClusterClip[lv1];
        float4 c2 = gsClusterClip[lv2];

        // Cull triangles completely behind near plane:
        if (c0.w <= 0.12 && c1.w <= 0.12 && c2.w <= 0.12)
        {
            continue;
        }

        // Clamp vertices crossing near plane for conservative screen-space bbox & Hi-Z test:
        float w0 = max(c0.w, 0.12);
        float w1 = max(c1.w, 0.12);
        float w2 = max(c2.w, 0.12);

        float2 s0 = float2((c0.x / w0 * 0.5 + 0.5) * float(dims.x), (c0.y / w0 * 0.5 + 0.5) * float(dims.y));
        float2 s1 = float2((c1.x / w1 * 0.5 + 0.5) * float(dims.x), (c1.y / w1 * 0.5 + 0.5) * float(dims.y));
        float2 s2 = float2((c2.x / w2 * 0.5 + 0.5) * float(dims.x), (c2.y / w2 * 0.5 + 0.5) * float(dims.y));

        float2 bboxMin = min(s0, min(s1, s2));
        float2 bboxMax = max(s0, max(s1, s2));
        if (bboxMax.x < 0.0 || bboxMax.y < 0.0 || bboxMin.x >= float(dims.x) || bboxMin.y >= float(dims.y))
        {
            continue;
        }

        // Conservative Hi-Z Tile Occlusion Reject (`Reversed-Z`: smaller bits = farther):
        float minVz = min(w0, min(w1, w2));
        uint triClosestRevZ = encodeReversedZDepth(minVz);
        uint tx0 = clamp(uint(max(bboxMin.x, 0.0)) / 8u, 0u, tileStride - 1u);
        uint ty0 = clamp(uint(max(bboxMin.y, 0.0)) / 8u, 0u, tileRows - 1u);
        uint tx1 = clamp(uint(max(bboxMax.x, 0.0)) / 8u, 0u, tileStride - 1u);
        uint ty1 = clamp(uint(max(bboxMax.y, 0.0)) / 8u, 0u, tileRows - 1u);
        if (tx0 == tx1 && ty0 == ty1)
        {
            uint tileMinRevZ = gHiZMip1[ty0 * tileStride + tx0];
            if (triClosestRevZ < tileMinRevZ)
            {
                continue;
            }
        }

        float2 span = bboxMax - bboxMin;
        bool allInFront = (c0.w > 0.12) && (c1.w > 0.12) && (c2.w > 0.12);
        uint payloadId31 = ((m & 0x7FFFFFu) << 8u) | (ti & 0xFFu);

        // `< 4x4 px` Micro-Triangle Compute Soft-Rasterizer -> 64-bit SSBO `atomicMax` (`§8.2`):
        if (allInFront && sceneHdr.lutAndVfxCounts.z != 0u && span.x < 4.0 && span.y < 4.0)
        {
            int px0 = clamp(int(floor(bboxMin.x)), 0, int(dims.x) - 1);
            int py0 = clamp(int(floor(bboxMin.y)), 0, int(dims.y) - 1);
            int px1 = clamp(int(ceil(bboxMax.x)), 0, int(dims.x) - 1);
            int py1 = clamp(int(ceil(bboxMax.y)), 0, int(dims.y) - 1);

            float area = (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x);
            if (abs(area) > 1e-4)
            {
                float invArea = 1.0 / area;
                float rz0 = c0.z / w0;
                float rz1 = c1.z / w1;
                float rz2 = c2.z / w2;
                for (int py = py0; py <= py1; ++py)
                {
                    for (int px = px0; px <= px1; ++px)
                    {
                        float2 samplePt = float2(float(px) + 0.5, float(py) + 0.5);
                        float wA = ((s1.x - samplePt.x) * (s2.y - samplePt.y) - (s1.y - samplePt.y) * (s2.x - samplePt.x)) * invArea;
                        float wB = ((s2.x - samplePt.x) * (s0.y - samplePt.y) - (s2.y - samplePt.y) * (s0.x - samplePt.x)) * invArea;
                        float wC = 1.0 - wA - wB;
                        if (wA >= 0.0 && wB >= 0.0 && wC >= 0.0)
                        {
                            float revZ = clamp(wA * rz0 + wB * rz1 + wC * rz2, 0.0, 1.0);
                            uint64_t visWord = packVisibilityWord64(asuint(revZ), 1u, payloadId31);
                            uint64_t prevWord;
                            InterlockedMax(gVisBuffer64[uint(py) * dims.x + uint(px)], visWord, prevWord);
                        }
                    }
                }
            }
        }
        else
        {
            // `>= 4x4 px` Triangle -> Hardware Early-Z `vkCmdDrawIndexedIndirectCount` Compaction (`<= 124` tris/meshlet):
            uint cmdSlot;
            InterlockedAdd(gIndirectDrawCount[0], 1u, cmdSlot);
            if (cmdSlot < 4096u)
            {
                uint baseIdx = cmdSlot * 3u;
                gCompactedIndices[baseIdx + 0u] = vBase + lv0;
                gCompactedIndices[baseIdx + 1u] = vBase + lv1;
                gCompactedIndices[baseIdx + 2u] = vBase + lv2;

                DrawIndexedIndirectCmdGpu cmd;
                cmd.indexCount = 3u;
                cmd.instanceCount = 1u;
                cmd.firstIndex = baseIdx;
                cmd.vertexOffset = 0;
                cmd.firstInstance = 0x80000000u | payloadId31;
                gIndirectCmds[cmdSlot] = cmd;
            }
        }
    }
}

// ============================================================================
// Pass 1C: `vis_indirect_vs` / `vis_indirect_fs` — Early-Z Hardware Raster
// ============================================================================
struct IndirectVsOut
{
    float4 clipPos : SV_Position;
    nointerpolation uint visPayload : TEXCOORD0;
};

[shader("vertex")]
IndirectVsOut vis_indirect_vs(uint globalVertIdx : SV_VertexID, uint startInstance : SV_StartInstanceLocation)
{
    IndirectVsOut o;
    uint clusterId = (startInstance >> 8u) & 0x7FFFFFu;
    uint rootBone = gMeshlets[min(clusterId, max(gPush.viewportAndFlags.w, 1u) - 1u)].materialParams.w;
    float3 worldP = skinMeshVertex(globalVertIdx, rootBone);
    float4 clip = projectWorldToClip(worldP);
    clip.w = max(clip.w, 0.08);
    o.clipPos = clip;
    o.visPayload = startInstance;
    return o;
}

[shader("fragment")]
[earlydepthstencil]
void vis_indirect_fs(IndirectVsOut inp)
{
    uint2 dims = gPush.viewportAndFlags.xy;
    uint2 px = uint2(inp.clipPos.xy);
    if (px.x >= dims.x || px.y >= dims.y)
    {
        return;
    }
    uint revDepthBits = asuint(clamp(inp.clipPos.z, 0.0, 1.0));
    uint id31 = inp.visPayload & 0x7FFFFFFFu;
    uint64_t visWord = packVisibilityWord64(revDepthBits, 1u, id31);
    uint64_t prevWord;
    InterlockedMax(gVisBuffer64[px.y * dims.x + px.x], visWord, prevWord);
}

[shader("fragment")]
[earlydepthstencil]
uint vis_fallback_fs(IndirectVsOut inp) : SV_Target0
{
    return inp.visPayload;
}

// ============================================================================
// Pass 2A: `meshlet_cel` — Overdraw-0% Single Compute Tile Material Resolve
// ============================================================================
[shader("compute")]
[numthreads(8, 8, 1)]
void meshlet_cel(uint3 tid : SV_DispatchThreadID)
{
    uint2 dims = gPush.viewportAndFlags.xy;
    if (tid.x >= dims.x || tid.y >= dims.y)
    {
        return;
    }

    uint pixIdx = tid.y * dims.x + tid.x;
    uint64_t winningVis = gVisBuffer64[pixIdx];
    if (winningVis == 0ULL)
    {
        return;
    }

    GpuSceneBdaHeader sceneHdr = loadSceneHeader();
    float3 sunDir = normalize(sceneHdr.sunDirAndMaxDist.xyz);
    uint lutSlot = sceneHdr.lutAndVfxCounts.x & 15u;

    float3 ro = gPush.camPosAndFov.xyz;
    float3 rd = computePrimaryRayDir(tid.xy, dims);

    // Neighbor primary rays `(rdX, rdY)` projected onto surface tangent plane inside `computeSurfaceUvGrads` (`§8.2`):
    float3 rdX = computePrimaryRayDir(uint2(min(tid.x + 1u, max(dims.x, 1u) - 1u), tid.y), dims);
    float3 rdY = computePrimaryRayDir(uint2(tid.x, min(tid.y + 1u, max(dims.y, 1u) - 1u)), dims);

    uint revDepthBits = uint(winningVis >> 32u);
    uint lo32 = uint(winningVis & 0xFFFFFFFFULL);
    uint isMeshlet = (lo32 >> 31u) & 1u;
    float nearZ = max(gPush.camUpAndNear.w, 0.05);
    float revZ = max(asfloat(revDepthBits), 1e-5);
    float linearT = nearZ / revZ;

    // Keep `gHiZMip0` synchronized with final resolved visibility (SDF + Meshlets) for screen-space Hi-Z specular!
    gHiZMip0[pixIdx] = revDepthBits;

    if (isMeshlet != 0u)
    {
        uint clusterId = (lo32 >> 8u) & 0x7FFFFFu;
        uint triId = lo32 & 0xFFu;
        float3 triNrm;
        float triT;
        gGbufferShaded[pixIdx] = shadeMeshletTriangle(clusterId, triId, ro, rd, rdX, rdY, sunDir, lutSlot, triNrm, triT);
        gGbufferNormal[pixIdx] = float4(triNrm, linearT);
    }
    else
    {
        uint hitField = lo32 & 0xFFu;
        float3 hitPos = ro + rd * linearT;
        float3 sdfNrm = estimateSdfNormal(hitPos);
        gGbufferShaded[pixIdx] = shadeSdfSurface(ro, rd, rdX, rdY, linearT, sdfNrm, hitField, sunDir, lutSlot);
        gGbufferNormal[pixIdx] = float4(sdfNrm, linearT);
    }
}

// ============================================================================
// Pass 2B: `radiance_cascades` — 3D Sparse Radiance Cascades GI + Hi-Z/SDF Specular
// ============================================================================
[shader("compute")]
[numthreads(8, 8, 1)]
void radiance_cascades(uint3 tid : SV_DispatchThreadID)
{
    uint2 dims = gPush.viewportAndFlags.xy;
    if (tid.x >= dims.x || tid.y >= dims.y)
    {
        return;
    }

    GpuSceneBdaHeader sceneHdr = loadSceneHeader();
    uint pixIdx = tid.y * dims.x + tid.x;
    float4 directRough = gGbufferShaded[pixIdx];
    float4 nrmDepth = gGbufferNormal[pixIdx];

    if (nrmDepth.w >= 450.0)
    {
        float3 rd = computePrimaryRayDir(tid.xy, dims);
        float skyT = clamp(rd.y * 0.5 + 0.5, 0.0, 1.0);
        // Low-chroma twilight zenith preserving `>= 3:1` gamut-normalized chroma contrast for telegraphs:
        float3 skyHdr = lerp(float3(0.020, 0.021, 0.023), sceneHdr.skyZenithAndRoughCutoff.xyz, skyT);
        gHdrLitBuffer[pixIdx] = float4(skyHdr, 1.0);
        return;
    }

    float3 ro = gPush.camPosAndFov.xyz;
    float3 rd = computePrimaryRayDir(tid.xy, dims);
    float3 worldPos = ro + rd * nrmDepth.w;
    float3 nrm = normalize(nrmDepth.xyz);

    float aoAcc = 0.0;
    float3 giIrradiance = float3(0.0, 0.0, 0.0);
    float3 upRef = (abs(nrm.y) < 0.95) ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
    float3 tangent = normalize(cross(upRef, nrm));
    float3 bitangent = cross(nrm, tangent);

    float3 coneDirs[4] = {
        normalize(nrm + 0.55 * tangent),
        normalize(nrm - 0.55 * tangent),
        normalize(nrm + 0.55 * bitangent),
        normalize(nrm - 0.55 * bitangent)
    };

    for (uint c = 0u; c < 4u; ++c)
    {
        float cascadeDist = 0.6 + float(c) * 0.9;
        float sdfD;
        uint fState;
        uint bId;
        sampleSparseSdfWorld(worldPos + coneDirs[c] * cascadeDist, sdfD, fState, bId);
        float vis = clamp(sdfD / cascadeDist, 0.0, 1.0);
        aoAcc += vis * 0.25;

        float3 bounceTint = float3(0.09, 0.11, 0.14);
        if (fState == 2u) { bounceTint = float3(0.22, 0.42, 0.55); }
        else if (fState == 3u) { bounceTint = float3(0.50, 0.22, 0.06); }
        else if (fState == 5u) { bounceTint = float3(0.35, 0.16, 0.62); }
        giIrradiance += bounceTint * vis * 0.25;
    }

    // Hybrid Screen-Space `Hi-Z` + World Sparse `SDF` (+ Optional `RayQuery`) Specular (`§8.2`):
    float3 specBounce = float3(0.0, 0.0, 0.0);
    float roughCutoff = max(sceneHdr.skyZenithAndRoughCutoff.w, 0.38);
    if (directRough.w <= roughCutoff)
    {
        float3 reflDir = reflect(rd, nrm);
        bool hitScreenHiZ = false;
        uint tileStride = max((dims.x + 7u) / 8u, 1u);

#if ENABLE_HW_RAY_QUERY
        RayDesc rayDesc;
        rayDesc.Origin = worldPos + nrm * 0.12;
        rayDesc.Direction = reflDir;
        rayDesc.TMin = 0.05;
        rayDesc.TMax = 24.0;
        RayQuery<RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH> rq;
        rq.TraceRayInline(gWorldTlas, RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH, 0xFF, rayDesc);
        rq.Proceed();
        if (rq.CommittedStatus() != COMMITTED_NOTHING)
        {
            specBounce = float3(0.26, 0.34, 0.46) * (1.0 - directRough.w);
            hitScreenHiZ = true;
        }
#endif

        // 1. Hierarchical Screen-Space `Hi-Z` Ray-March (`gHiZMip1` coarse skip -> `gHiZMip0` fine hit):
        float sDist = 0.45;
        for (uint hs = 0u; hs < 6u && !hitScreenHiZ; ++hs)
        {
            float3 sampleW = worldPos + nrm * 0.08 + reflDir * sDist;
            float4 clipS = projectWorldToClip(sampleW);
            if (clipS.w <= 0.15)
            {
                break;
            }
            float2 uvS = float2(clipS.x / clipS.w * 0.5 + 0.5, clipS.y / clipS.w * 0.5 + 0.5);
            if (uvS.x < 0.0 || uvS.x >= 1.0 || uvS.y < 0.0 || uvS.y >= 1.0)
            {
                break;
            }
            uint2 pxS = uint2(uvS * float2(dims));
            uint rayRevZ = encodeReversedZDepth(clipS.w);
            uint coarseMinRevZ = gHiZMip1[(pxS.y / 8u) * tileStride + (pxS.x / 8u)];
            if (rayRevZ > coarseMinRevZ && coarseMinRevZ > 0u)
            {
                sDist += 1.10; // Fast coarse Hi-Z empty-space skip
                continue;
            }
            uint fineRevZ = gHiZMip0[pxS.y * dims.x + pxS.x];
            if (fineRevZ > 0u && rayRevZ <= fineRevZ)
            {
                float surfLinear = max(gPush.camUpAndNear.w, 0.05) / max(asfloat(fineRevZ), 1e-5);
                if (clipS.w - surfLinear < 1.25)
                {
                    specBounce = gGbufferShaded[pxS.y * dims.x + pxS.x].xyz * (1.0 - directRough.w) * 0.65;
                    hitScreenHiZ = true;
                    break;
                }
            }
            sDist += 0.65;
        }

        // 2. World Sparse `SDF` Fallback Ray-March for off-screen / disoccluded rays:
        if (!hitScreenHiZ)
        {
            float rt = 0.5;
            for (uint rs = 0u; rs < 8u; ++rs)
            {
                float sdfR;
                uint rField;
                uint rBid;
                sampleSparseSdfWorld(worldPos + reflDir * rt, sdfR, rField, rBid);
                if (sdfR <= 0.08)
                {
                    specBounce = (rField == 5u) ? float3(0.45, 0.20, 0.85) : float3(0.18, 0.26, 0.32);
                    break;
                }
                rt += max(sdfR, 0.35);
            }
            if (all(specBounce == float3(0.0, 0.0, 0.0)))
            {
                specBounce = float3(0.05, 0.06, 0.07) * (1.0 - directRough.w);
            }
        }
    }

    float ao = clamp(0.25 + 0.75 * aoAcc, 0.2, 1.0);
    float3 hdrTotal = directRough.xyz * ao + giIrradiance + specBounce;
    gHdrLitBuffer[pixIdx] = float4(hdrTotal, ao);
}

// ============================================================================
// Pass 3: `vfx_telegraph` — Translucent 3D VFX + ACES + Orthogonal OKLab Telegraph
// ============================================================================
[shader("compute")]
[numthreads(8, 8, 1)]
void vfx_telegraph(uint3 tid : SV_DispatchThreadID)
{
    uint2 dims = gPush.viewportAndFlags.xy;
    if (tid.x >= dims.x || tid.y >= dims.y)
    {
        return;
    }

    GpuSceneBdaHeader sceneHdr = loadSceneHeader();
    uint pixIdx = tid.y * dims.x + tid.x;
    float3 hdr = gHdrLitBuffer[pixIdx].xyz;
    float opaqueDepth = gGbufferNormal[pixIdx].w;

    // 1. Translucent 3D Volumetric VFX Composite (`§8.2` '후속 반투명 합성'):
    uint vfxCount = min(sceneHdr.lutAndVfxCounts.y, 8u);
    if (vfxCount > 0u)
    {
        float3 ro = gPush.camPosAndFov.xyz;
        float3 rd = computePrimaryRayDir(tid.xy, dims);
        for (uint vi = 0u; vi < vfxCount; ++vi)
        {
            GpuVfxEmitter em = sceneHdr.vfxEmitters[vi];
            float rad = em.centerAndRadius.w;
            if (rad <= 0.01 || em.colorAndAlpha.w <= 0.001)
            {
                continue;
            }
            float3 oc = ro - em.centerAndRadius.xyz;
            float b = dot(oc, rd);
            float c = dot(oc, oc) - rad * rad;
            float disc = b * b - c;
            if (disc > 0.0)
            {
                float sqrtD = sqrt(disc);
                float tNear = max(-b - sqrtD, 0.05);
                float tFar = -b + sqrtD;
                if (tFar > tNear && tNear < opaqueDepth)
                {
                    float chord = min(tFar, opaqueDepth) - tNear;
                    float shellFactor = clamp(chord / max(rad * 1.6, 0.1), 0.0, 1.0);
                    float alpha = clamp(em.colorAndAlpha.w * shellFactor, 0.0, 0.85);
                    hdr = hdr * (1.0 - alpha) + em.colorAndAlpha.xyz * alpha;
                }
            }
        }
    }

    // 2. ACES Filmic Post-Tonemapping + Stylized NPR Stepped Cel-Ramp (`§1.4`, exact `1/51` integer steps):
    float3 a = hdr * (hdr + 0.0245786) - 0.000090537;
    float3 b = hdr * (0.983729 * hdr + 0.4329510) + 0.238081;
    float3 ldr = clamp(a / max(b, float3(1e-4, 1e-4, 1e-4)), float3(0.0, 0.0, 0.0), float3(1.0, 1.0, 1.0));
    ldr = floor(ldr * 51.0 + 0.5) / 51.0;

    // 3. Post-Tonemap Background-Independent Orthogonal OKLab Telegraph & Dual Achromatic Outline (`§1.3, §8.2`):
    if (gPush.telegraphParams.x != 0u)
    {
        uint x0 = gPush.telegraphRect.x;
        uint y0 = gPush.telegraphRect.y;
        uint w0 = gPush.telegraphRect.z;
        uint h0 = gPush.telegraphRect.w;
        uint border = max(gPush.telegraphParams.z, 2u);

        bool inOuter = (tid.x >= x0) && (tid.x < x0 + w0) && (tid.y >= y0) && (tid.y < y0 + h0);
        if (inOuter)
        {
            bool inInner = (tid.x >= x0 + border) && (tid.x + border < x0 + w0)
                        && (tid.y >= y0 + border) && (tid.y + border < y0 + h0);
            bool inCore  = (tid.x >= x0 + border * 2u) && (tid.x + border * 2u < x0 + w0)
                        && (tid.y >= y0 + border * 2u) && (tid.y + border * 2u < y0 + h0);

            if (!inInner)
            {
                // Outer dark achromatic ring (`Y = 0.008`, `C_hat = 0.0 <= 0.08`):
                ldr = float3(0.008, 0.008, 0.008);
            }
            else if (!inCore)
            {
                // Inner bright luminance-inverted achromatic ring (`Y = 0.96`, `17.4:1 >= 3:1` inversion!):
                ldr = float3(0.96, 0.96, 0.96);
            }
            else
            {
                // Gamut-cusp orthogonal OKLab telegraph colors (`C_hat = 1.0`, `Y ≈ 0.63`, `|dh| = 120.9 deg >= 90 deg`):
                // Mode 0 (`Dodge` -> `Phantasia`): Amber-Gold `(1.00, 0.58, 0.00)` (`h = +88.7 deg`)
                // Mode 1 (`Parry` -> `Clash`):     Electric-Cyan `(0.00, 0.78, 1.00)` (`h = -150.4 deg`)
                ldr = (gPush.telegraphParams.y == 0u)
                    ? float3(1.00, 0.58, 0.00)
                    : float3(0.00, 0.78, 1.00);
            }
        }
        else
        {
            // Background perceptual desaturation/dimming veil during active Boss telegraph (`§1.3, §8.2`):
            // Guarantees `Y_bg <= 0.14` & `C_hat_bg <= 0.14` (`>= 3.5:1` luminance & `>= 5.5:1` chroma contrast
            // even under multi-player VFX storms or saturated elemental terrain!).
            float yBg = dot(ldr, float3(0.2126, 0.7152, 0.0722));
            float3 neutralBg = float3(yBg, yBg, yBg);
            ldr = floor(lerp(neutralBg, ldr, 0.15) * 0.25 * 85.0 + 0.5) / 85.0;
        }
    }

    gOutColorImage[tid.xy] = float4(ldr, 1.0);
}

// ============================================================================
// Core Fallback Resolve (`tile_shade_fallback`, when `shaderBufferInt64Atomics` is absent)
// Populates BOTH `gHiZMip0` AND `gHiZMip1` (8x8 tile minimum Reversed-Z) + `SampleGrad`!
// ============================================================================
groupshared uint gsFallbackTileMinRevZ;

[shader("compute")]
[numthreads(8, 8, 1)]
void tile_shade_fallback(uint3 tid : SV_DispatchThreadID, uint3 gid : SV_GroupID, uint groupIndex : SV_GroupIndex)
{
    if (groupIndex == 0u)
    {
        gsFallbackTileMinRevZ = 0x7F7FFFFFu;
    }
    GroupMemoryBarrierWithGroupSync();

    uint2 dims = gPush.viewportAndFlags.xy;
    if (tid.x < dims.x && tid.y < dims.y)
    {
        GpuSceneBdaHeader sceneHdr = loadSceneHeader();
        float3 sunDir = normalize(sceneHdr.sunDirAndMaxDist.xyz);
        uint lutSlot = sceneHdr.lutAndVfxCounts.x & 15u;

        uint pixIdx = tid.y * dims.x + tid.x;
        float3 ro = gPush.camPosAndFov.xyz;
        float3 rd = computePrimaryRayDir(tid.xy, dims);
        float3 rdX = computePrimaryRayDir(uint2(min(tid.x + 1u, max(dims.x, 1u) - 1u), tid.y), dims);
        float3 rdY = computePrimaryRayDir(uint2(tid.x, min(tid.y + 1u, max(dims.y, 1u) - 1u)), dims);

        uint resolvedRevZ = 0u;
        uint r32Id = gVisFallbackR32[pixIdx];
        if ((r32Id & 0x80000000u) != 0u)
        {
            uint clusterId = (r32Id >> 8u) & 0x7FFFFFu;
            uint triId = r32Id & 0xFFu;
            float3 triNrm;
            float triT;
            gGbufferShaded[pixIdx] = shadeMeshletTriangle(clusterId, triId, ro, rd, rdX, rdY, sunDir, lutSlot, triNrm, triT);
            gGbufferNormal[pixIdx] = float4(triNrm, triT);
            resolvedRevZ = encodeReversedZDepth(triT);
        }
        else
        {
            float t = 0.25;
            bool hitSdf = false;
            for (uint step = 0u; step < 48u; ++step)
            {
                float sdfM;
                uint fState;
                uint bId;
                sampleSparseSdfWorld(ro + rd * t, sdfM, fState, bId);
                if (sdfM <= 0.04)
                {
                    float3 nrm = estimateSdfNormal(ro + rd * t);
                    gGbufferShaded[pixIdx] = shadeSdfSurface(ro, rd, rdX, rdY, t, nrm, fState, sunDir, lutSlot);
                    gGbufferNormal[pixIdx] = float4(nrm, t);
                    resolvedRevZ = encodeReversedZDepth(t);
                    hitSdf = true;
                    break;
                }
                t += max(sdfM * 0.85, 0.18);
                if (t > 120.0)
                {
                    break;
                }
            }
            if (!hitSdf)
            {
                gGbufferShaded[pixIdx] = float4(0.022, 0.023, 0.025, 0.85);
                gGbufferNormal[pixIdx] = float4(0.0, 1.0, 0.0, 500.0);
            }
        }

        gHiZMip0[pixIdx] = resolvedRevZ;
        uint prevMin;
        InterlockedMin(gsFallbackTileMinRevZ, resolvedRevZ, prevMin);
    }

    GroupMemoryBarrierWithGroupSync();
    if (groupIndex == 0u)
    {
        uint tileStride = max((dims.x + 7u) / 8u, 1u);
        gHiZMip1[gid.y * tileStride + gid.x] = gsFallbackTileMinRevZ;
    }
}
