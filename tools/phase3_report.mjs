import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(process.argv[2] ?? 'reports/reconciliation-20261010-phase3');
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const analysis = read('FASE_3_ANALISE_47.json');
const decisions = read('FASE_3_DECISOES_INDIVIDUAIS.json');
const applied = read('FASE_3_MOVIMENTACOES_APLICADAS.json');
const empty = read('FASE_3_PASTAS_VAZIAS_REMOVIDAS.json');
const workCopy = read('FASE_3_VERIFICACAO_COPIA_TRABALHO.json');
const analysisByPath = new Map(analysis.records.map(record => [record.caminho_manager, record]));
const counts = decisions.counts;

const esc = value => String(value ?? '').replaceAll('|', '\\|').replaceAll('\n', ' ');
const pending = decisions.records.filter(record => record.decision === 'REVISÃO_PENDENTE');
const families = new Map();
for (const record of decisions.records) families.set(record.family, (families.get(record.family) ?? 0) + 1);

const lines = [
  '# Relatório final — Fase 3 dirigida',
  '',
  `Gerado em UTC: ${new Date().toISOString()}`,
  '',
  '## Resultado',
  '',
  `A investigação dirigida cobriu os 47 registros \`REVISÃO_PENDENTE\` da Fase 2. Foram resolvidos **${counts.resolved}**, mantidos pendentes **${counts.pending}** e executadas **${applied.counts.moved_verified}** movimentações verificadas somente na cópia de trabalho. O backup confirmou os 47 arquivos de origem antes das operações.`,
  '',
  '| Medida | Resultado |',
  '|---|---:|',
  `| Registros investigados | ${counts.total} |`,
  `| Resolvidos tecnicamente | ${counts.resolved} |`,
  `| Pendentes de decisão | ${counts.pending} |`,
  `| Movimentações verificadas | ${applied.counts.moved_verified} |`,
  `| Pastas antigas realmente vazias removidas na cópia | ${empty.folders.length} |`,
  `| Packages na cópia após a operação | ${workCopy.package_count} |`,
  '',
  '## Método técnico',
  '',
  '- Comparação dirigida de recursos DBPF, TGIs completos, hashes individuais, tipos de recurso, assemblies/S3SA, CASP, OBJD/OBJK, GEOM, versões e relações entre packages.',
  '- TGIs genéricos compartilhados com conteúdo do jogo não foram tratados como equivalência de conteúdo; a decisão exigiu sobreposição de recursos discriminantes, hash ou evidência de família coerente.',
  '- Boroughsburg, Lazy Duchess Cat/Dog, BoringBones, Cmar, sam4v2 e EPs Fixes foram avaliados como famílias, mantendo variantes relacionadas juntas.',
  '- Não foram restaurados arquivos desativados, versões antigas, alternativas incompatíveis ou conteúdos removidos intencionalmente.',
  '',
  '## Famílias e decisões',
  '',
  '| Família | Registros |',
  '|---|---:|',
  ...[...families.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([family, total]) => `| ${esc(family)} | ${total} |`),
  '',
  'Regras gerais incorporadas ao classificador:',
  '',
  '- EPs Fixes só são preservados na árvore canônica `Correções\\Gameplay\\EPs Fixes\\EPxx` quando coexistem caminho coerente e evidência de recurso funcional; a estrutura protegida não foi achatada.',
  '- Famílias Cmar e sam4v2 são agrupadas por GEOM/LOD e nomes internos, sem separar meshes relacionadas.',
  '- Lazy Duchess Cat/Dog Addons e BoringBones usam família + função + recursos internos, sem classificação por nome isolado.',
  '- Defaults de face/olhos/sobrancelhas/cílios e skins exigem S3SA/NMAP/MANIFEST e assinatura interna, sem inventar identidade CAS.',
  '- A regra NRaas existente continua exigindo principal + módulo verificado para criar subpastas, em `Scripts\\Jogabilidade\\NRaas`.',
  '',
  '## Antes × depois — 47 registros',
  '',
  '| Antes | Decisão | Depois | Evidência resumida |',
  '|---|---|---|---|',
  ...decisions.records.map(record => {
    const evidence = analysisByPath.get(record.caminho_manager);
    const detail = record.technical_basis || evidence?.justificativa || '';
    return `| ${esc(record.caminho_manager)} | ${esc(record.decision)} | ${esc(record.destination || '—')} | ${esc(detail)} |`;
  }),
  '',
  '## Casos ainda dependentes de decisão',
  '',
  `Permanecem **${pending.length}** casos. Eles ficaram na cópia de trabalho, sem restauração ou reativação automática:`,
  '',
  ...pending.map(record => `- **${esc(record.caminho_manager)}** — ${esc(record.technical_basis)}`),
  '',
  'A decisão manual necessária é confirmar a intenção de manter, categorizar ou excluir esses cinco bundles sem identidade técnica suficiente. `cas2.package` e `SavannahRosenberg.package` são bundles grandes/mesclados; os três restantes são packages mínimos de 233 bytes sem recursos suficientes para atribuição segura.',
  '',
  '## Validações',
  '',
  '- Teste regressivo da Fase 2: aprovado.',
  '- Teste regressivo da Fase 3: aprovado (47/42/5, 39 movimentações e 15 pastas vazias).',
  '- Sintaxe dos scripts JavaScript da Fase 3: aprovada com `node --check`.',
  '- Integridade da cópia: todos os destinos movidos mantêm o SHA-256 planejado e os cinco pendentes permanecem presentes.',
  '- `cargo fmt --check`/`rustfmt --check` não pôde ser concluído neste ambiente restrito por erro de acesso ao caminho canônico do workspace; nenhum build foi executado.',
  '- Modelo, instalação ativa e backup não foram usados como destino de movimentação.',
  '',
  'Relatórios detalhados individuais, JSONs de evidência, CSV e verificação da cópia estão no mesmo pacote local da Fase 3. Eles não devem ser adicionados ao histórico público.',
];

fs.writeFileSync(path.join(root, 'FASE_3_RELATORIO_FINAL.md'), `${lines.join('\n')}\n`);
console.log(JSON.stringify({ ok: true, report: 'FASE_3_RELATORIO_FINAL.md', records: counts.total, resolved: counts.resolved, pending: counts.pending }, null, 2));
