import fs from 'node:fs/promises';
import path from 'node:path';

const root = path.resolve(process.argv[2] ?? 'reports/reconciliation-20261010-phase3');
const analysis = JSON.parse(await fs.readFile(path.join(root, 'FASE_3_ANALISE_47.json'), 'utf8'));
const basename = value => value.split('/').at(-1);
const byPath = new Map(analysis.records.map(record => [record.caminho_manager, record]));
const unresolvedBasis = record => {
  const pathName = record.caminho_manager;
  if (pathName === 'cas2.package') {
    return `Bundle mesclado de ${record.manager_size} bytes e ${record.manager_resource_count} recursos (${record.manager_resource_types.join(', ')}), sem assembly identificado. Os melhores candidatos compartilham apenas 7–8 TGIs genéricos, têm Jaccard inferior a 0,02 e no máximo 1 hash de recurso; não é seguro atribuir, substituir ou excluir automaticamente.`;
  }
  if (pathName === 'SavannahRosenberg.package') {
    return `Bundle mesclado de ${record.manager_size} bytes e ${record.manager_resource_count} recursos (${record.manager_resource_types.join(', ')}), sem assembly identificado. Os melhores candidatos compartilham apenas 8–15 TGIs genéricos, Jaccard inferior a 0,02 e no máximo 1 hash de recurso; não há identidade de família ou versão comprovada.`;
  }
  if (pathName.includes('project_moondust')) {
    return `Package de ${record.manager_size} bytes com ${record.manager_resource_count} recursos e apenas tipos genéricos (${record.manager_resource_types.join(', ')}). Os candidatos mais próximos compartilham 2 TGIs genéricos, nenhum hash e nenhuma assinatura de família; não é seguro reativar, categorizar ou excluir.`;
  }
  if (pathName.includes('amTopEP09ActivistRebel')) {
    return `Package mínimo de ${record.manager_size} bytes contendo somente 1 S3SA (TGI ${record.instances.split(';')[0]}), sem assembly, hash correspondente ou conteúdo de catálogo. O nome sugere EP09/Fix, mas não prova função nem substituto; exige decisão manual.`;
  }
  if (pathName.includes('amTopJacketSmoking')) {
    return `Package mínimo de ${record.manager_size} bytes contendo somente 1 S3SA (TGI ${record.instances.split(';')[0]}), sem assembly, hash correspondente ou conteúdo de catálogo. O nome identifica uma peça, mas não prova versão, categoria canônica ou intenção de remoção; exige decisão manual.`;
  }
  return record.justificativa;
};
const decisions = analysis.records.map(record => ({
  caminho_manager: record.caminho_manager,
  sha256: record.sha256,
  size: record.manager_size,
  decision: 'REVISÃO_PENDENTE',
  destination: null,
  action: 'none',
  family: 'unresolved',
  technical_basis: unresolvedBasis(record),
}));
const decisionByPath = new Map(decisions.map(record => [record.caminho_manager, record]));

function resolve(paths, decision, family, basis, destination = null) {
  for (const relative of paths) {
    const record = decisionByPath.get(relative);
    if (!record) throw new Error(`phase 3 rule did not match record: ${relative}`);
    record.decision = decision;
    record.family = family;
    record.technical_basis = basis;
    record.destination = destination ? (typeof destination === 'function' ? destination(relative) : destination) : null;
    record.action = record.destination ? 'move_verified_after_simulation' : 'keep_in_place';
  }
}

const pending = relative => analysis.records.filter(record => record.caminho_manager.includes(relative)).map(record => record.caminho_manager);

resolve(
  ['- Lazy Duchess/ld_RandomSimFixes.package'],
  'VERSÃO_ALTERNATIVA',
  'Lazy Duchess',
  'Os três TGIs completos coincidem com a versão da modelo, mas somente um hash de recurso coincide; tamanho, assembly PuddingFaceFix e conteúdo de tuning diferem. É uma alternativa/versionamento, não duplicata nem restauração.',
);
resolve(
  analysis.records.filter(record => /BuhudainDRYouAreRealV14FemaleKertinFace/.test(record.caminho_manager)).map(record => record.caminho_manager),
  'VERSÃO_ALTERNATIVA',
  'Buhudain skin family',
  'O conjunto de TGIs coincide integralmente com a alternativa da modelo; os nomes internos/arquivos indicam V14/V15 e os hashes dos recursos não coincidem. Manter a versão ativa, sem substituir por versão antiga.',
);

resolve(
  pending('Sem Categoria/! Fixes/EPs Fixes/EP01 - PotatoBalladSims - terraindistantFrance'),
  'MANTER_E_CATEGORIZAR',
  'EPs Fixes',
  'Caminho de origem identifica EPs Fixes, o pacote tem recursos DBPF de correção e não há hash duplicado. A ação apenas normaliza a árvore canônica EP01; não reativa nem substitui conteúdo.',
  relative => `Correções/Gameplay/EPs Fixes/EP01/${basename(relative)}`,
);
resolve(
  pending('Sem Categoria/Fixes/EPs Fixes/EP01 - PotatoBalladSims'),
  'MANTER_E_CATEGORIZAR',
  'EPs Fixes',
  'Família PotatoBalladSims/EP01 explicitamente identificada no caminho e no nome, com recurso de correção; normalização estrutural segura para a árvore canônica protegida.',
  relative => `Correções/Gameplay/EPs Fixes/EP01/${basename(relative)}`,
);

resolve(
  ['Sem Categoria/! Boroughsburg_Main_CC/PotatoBalladSims_IcicleFix.package'],
  'MANTER_E_CATEGORIZAR',
  'PotatoBalladSims fixes',
  'Nome explícito de fix, autor consistente, S3SA e evidência interna icicle/SnowSpecular/DDS; não há candidato TGI equivalente. Categorizar como correção comum, sem inventar EP.',
  'Correções/Gameplay/PotatoBalladSims_IcicleFix.package',
);
resolve(
  ['Sem Categoria/! Boroughsburg_Main_CC/PotatoBalladSims_IPConicalLightstorm_FTPTFix.package'],
  'MANTER_E_CATEGORIZAR',
  'PotatoBalladSims fixes',
  'Nome explícito de fix, autor consistente e recurso FTPT; não há candidato TGI equivalente. Categorizar como correção comum, sem mover para EPs Fixes sem EP comprovado.',
  'Correções/Gameplay/PotatoBalladSims_IPConicalLightstorm_FTPTFix.package',
);

resolve(
  analysis.records.filter(record => record.caminho_manager.startsWith('! Boroughsburg_Main_CC/')).map(record => record.caminho_manager),
  'MANTER_E_CATEGORIZAR',
  'Boroughsburg family bundle',
  'Os seis packages formam um conjunto autoral coerente: CASP, objetos/OBJD, rabbit holes, world/build content e um disabler de efeito. Os poucos TGIs compartilhados são recursos-base do jogo, não equivalência de conteúdo. A família é mantida agrupada por subpasta Boroughsburg.',
  relative => {
    if (relative.endsWith('Boroughsburg_CASP.package')) return `CAS/Boroughsburg/${basename(relative)}`;
    if (relative.endsWith('Boroughsburg_Rabbitholes.package')) return `Construção/Rabbit Holes/Boroughsburg/${basename(relative)}`;
    if (relative.endsWith('Boroughsburg_WorldContent.package')) return `Construção/Boroughsburg/${basename(relative)}`;
    if (relative.endsWith('TrainSmokeDisabler.package')) return `Substituições/Efeitos/Boroughsburg/${basename(relative)}`;
    return `Objetos/Boroughsburg/${basename(relative)}`;
  },
);
resolve(
  ['Sem Categoria/! Boroughsburg_Main_CC/PotatoBalladSims_Boroughsburg_TrainSmokeDisabler.package'],
  'MANTER_E_CATEGORIZAR',
  'Boroughsburg family bundle',
  'Disabler explícito da mesma família Boroughsburg; o recurso único não coincide com candidato da modelo, portanto é mantido como substituição de efeito dentro da família, sem separação como conteúdo independente.',
  'Substituições/Efeitos/Boroughsburg/PotatoBalladSims_Boroughsburg_TrainSmokeDisabler.package',
);

resolve(
  pending('Sem Categoria/- Lazy Duchess/Cat and Dog Addons/'),
  'MANTER_E_CATEGORIZAR',
  'Lazy Duchess Cat/Dog Addons',
  'Os três packages compartilham a família Cat and Dog Addons e evidência interna de malhas/texturas corporais; não há TGI equivalente. Manter o conjunto sob uma família de substituições de corpo, sem separar arquivos relacionados.',
  relative => `Substituições/Corpo/Cães e Gatos/Lazy Duchess/${basename(relative)}`,
);
resolve(
  pending('Sem Categoria/Boring Bones/'),
  'MANTER_E_CATEGORIZAR',
  'BoringBones family bundle',
  'A família contém um mod de iluminação, uma substituição ocular e quatro substituições de vegetação, todas com evidência interna compatível. A separação é apenas por função, preservando BoringBones como subpasta.',
  relative => {
    const name = basename(relative);
    if (/Ethereal-Glow-LightingMod/i.test(name)) return `Scripts/Jogabilidade/BoringBones/${name}`;
    if (/olhos/i.test(name)) return `Substituições/Corpo/Olhos/BoringBones/${name}`;
    return `Substituições/Ambiente/BoringBones/${name}`;
  },
);
resolve(
  pending('Sem Categoria/Cmar/'),
  'MANTER_E_CATEGORIZAR',
  'Cmar body family',
  'Os três packages têm GEOM e nomes internos am/em/tm de bulge/body mesh, pertencem ao mesmo conjunto BodyHairPubicMaleV3 e não têm substituto exato na modelo. Manter o conjunto unido em Genética/Genitálias/Masculina.',
  relative => `Genética/Genitálias/Masculina/Cmar/${basename(relative)}`,
);
resolve(
  pending('Sem Categoria/MTS_nanashihito_1377720_sam4v2/'),
  'MANTER_E_CATEGORIZAR',
  'sam4v2 coherent GEOM set',
  'Os seis packages são um conjunto GEOM coerente, com nomes am/em/tm e variantes LOD/burnt/pendant; não há CASP/OBJD nem equivalente exato. Mantém-se o conjunto junto, sem separar variantes LOD.',
  relative => `Genética/Corpo/Masculina/sam4v2/${basename(relative)}`,
);

resolve(
  ['Sem Categoria/#1 Face/Eyes/HIX_BeetleEyes_DEF.package'],
  'MANTER_E_CATEGORIZAR',
  'default eye replacement',
  'S3SA + caminho Eyes + sufixo DEF e evidência interna ocular; ausência de CASP/OBJD impede tratá-lo como roupa, mas sustenta substituição ocular.',
  'Substituições/Corpo/Olhos/HIX_BeetleEyes_DEF.package',
);
resolve(
  ['Sem Categoria/#1 Face/HIX_DefaultFaceOverlay.package'],
  'MANTER_E_CATEGORIZAR',
  'default face replacement',
  'S3SA + DefaultFaceOverlay identificam uma substituição de rosto; sem TGI equivalente e sem conteúdo de catálogo concorrente.',
  'Substituições/Corpo/Rosto/HIX_DefaultFaceOverlay.package',
);
resolve(
  ['Sem Categoria/#1 Face/imam_TS3_Squea_Dynastid-AGSclera_DEF.package'],
  'MANTER_E_CATEGORIZAR',
  'default eye replacement',
  'S3SA + AGSclera/DEF identificam substituição ocular; sem CASP/OBJD ou TGI equivalente.',
  'Substituições/Corpo/Olhos/imam_TS3_Squea_Dynastid-AGSclera_DEF.package',
);
resolve(
  ['Sem Categoria/#1 Face/Make Up/Eyelashes/S-Club #[ts3-eyelash] design-n3-a.package'],
  'MANTER_E_CATEGORIZAR',
  'eyelash family',
  'Caminho Make Up/Eyelashes e recurso S3SA indicam pacote de cílios; manter no conjunto de maquiagem sem inferir idade/gênero por nome.',
  relative => `CAS/Maquiagem/Cílios/${basename(relative)}`,
);
resolve(
  ['Sem Categoria/#4 Hair/Eyebrows/SLU_Sims3_DefaultBrowsAll.package'],
  'MANTER_E_CATEGORIZAR',
  'eyebrow family',
  'Caminho Eyebrows + DefaultBrows e S3SA; sobrancelhas permanecem na regra de Genética, sem idade.',
  relative => `Genética/Sobrancelhas/${basename(relative)}`,
);
resolve(
  pending('Sem Categoria/#5 Skin/'),
  'MANTER_E_CATEGORIZAR',
  'default skin replacements',
  'Os três packages têm S3SA, caminho Skin e nomes DEFAULT/skin, sem CASP/OBJD concorrente nem TGI equivalente; manter como substituições de pele.',
  relative => `Substituições/Corpo/Definição de Pele/${basename(relative)}`,
);
resolve(
  ['Sem Categoria/BrntWafflesSkyrimStarField2048.package'],
  'MANTER_E_CATEGORIZAR',
  'environment replacement',
  'S3SA + StarField2048 e evidência ambiental identificam substituição de céu/ambiente; não há candidato equivalente.',
  'Substituições/Ambiente/BrntWafflesSkyrimStarField2048.package',
);

const unresolved = decisions.filter(record => record.decision === 'REVISÃO_PENDENTE');
const moves = decisions.filter(record => record.destination);
await fs.writeFile(path.join(root, 'FASE_3_DECISOES_INDIVIDUAIS.json'), JSON.stringify({
  generated_at_utc: new Date().toISOString(),
  records: decisions,
  counts: { total: decisions.length, resolved: decisions.length - unresolved.length, pending: unresolved.length, moves: moves.length },
}, null, 2));
const csvCell = value => `"${String(value ?? '').replaceAll('"', '""')}"`;
const csv = ['caminho_manager,decision,destination,action,family,technical_basis'];
for (const record of decisions) csv.push([record.caminho_manager, record.decision, record.destination, record.action, record.family, record.technical_basis].map(csvCell).join(','));
await fs.writeFile(path.join(root, 'FASE_3_DECISOES_INDIVIDUAIS.csv'), `${csv.join('\n')}\n`);
await fs.writeFile(path.join(root, 'FASE_3_PLANO_MOVIMENTACOES.json'), JSON.stringify({
  generated_at_utc: new Date().toISOString(),
  operations: moves.map(record => ({ before: record.caminho_manager, after: record.destination, sha256: record.sha256, size: record.size, decision: record.decision, family: record.family, reason: record.technical_basis })),
  protected_pending: unresolved.map(record => ({ path: record.caminho_manager, reason: record.technical_basis })),
}, null, 2));
console.log(JSON.stringify({ total: decisions.length, resolved: decisions.length - unresolved.length, pending: unresolved.length, moves: moves.length }, null, 2));
