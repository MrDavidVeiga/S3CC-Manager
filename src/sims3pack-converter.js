import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

const PREFS_KEY = "s3cc-organizer-preferences-v1";

const TEXT = {
  en: {
    title:"Sims3Pack → Package",
    intro:"Extract embedded packages and give them readable names from the Sims3Pack manifest or CASP.",
    chooseFiles:"Choose Sims3Pack(s)", chooseDestination:"Choose Destination", convert:"Convert to .package",
    files:"Files", destination:"Destination", empty:"Choose one or more .Sims3Pack files to preview the conversion.",
    inspecting:"Reading Sims3Pack…", converting:"Converting…", ready:"Ready to convert",
    set:n=>`Set · ${n} packages`, single:"Single package", nonPackage:"Not a package",
    manifest:"Manifest name", sims3packManifest:"Sims3Pack localized name", casp:"CASP name",
    packaged:"Packaged file name", fallback:"Fallback name", invalid:"Invalid",
    converted:n=>`${n} package(s) converted`, skipped:n=>`${n} skipped`,
    noPackages:"No convertible .package payloads were found.", openDestination:"Open destination",
    warning:"Warning", source:"Name source", output:"Output", internal:"Internal file",
    combined:"Combined conversion", separateHint:"Unchecked: separate conversion",
    destinationHint:"Suggested destination", combinedUnsafe:"Combined conversion is unavailable for worlds, lots, Sims, Store Featured Sets and other protected content.",
    manifest_disambiguated:"Distinct internal manifest name",
    detectedType:"Detected type", storeSet:"Store Featured Set",
  },
  pt: {
    title:"Sims3Pack → Package",
    intro:"Extraia os packages internos usando nomes legíveis do manifest do Sims3Pack ou do CASP.",
    chooseFiles:"Escolher Sims3Pack(s)", chooseDestination:"Escolher Destino", convert:"Converter para .package",
    files:"Arquivos", destination:"Destino", empty:"Escolha um ou mais arquivos .Sims3Pack para visualizar a conversão.",
    inspecting:"Lendo Sims3Pack…", converting:"Convertendo…", ready:"Pronto para converter",
    set:n=>`Set · ${n} packages`, single:"Package único", nonPackage:"Não é package",
    manifest:"Nome do manifest", sims3packManifest:"Nome localizado do Sims3Pack", casp:"Nome do CASP",
    packaged:"Nome do arquivo interno", fallback:"Nome de fallback", invalid:"Inválido",
    converted:n=>`${n} package(s) convertido(s)`, skipped:n=>`${n} ignorado(s)`,
    noPackages:"Nenhum payload .package convertível foi encontrado.", openDestination:"Abrir destino",
    warning:"Aviso", source:"Origem do nome", output:"Saída", internal:"Arquivo interno",
    combined:"Conversão conjunta", separateHint:"Desmarcado: conversão separada",
    destinationHint:"Destino sugerido", combinedUnsafe:"A conversão conjunta não está disponível para mundos, lotes, Sims, conjuntos da Store e outros conteúdos protegidos.",
    manifest_disambiguated:"Nome interno distinto do manifesto",
    detectedType:"Tipo identificado", storeSet:"Conjunto da Store",
  },
  es: {
    title:"Sims3Pack → Package",
    intro:"Extrae los packages internos usando nombres legibles del manifest del Sims3Pack o del CASP.",
    chooseFiles:"Elegir Sims3Pack(s)", chooseDestination:"Elegir Destino", convert:"Convertir a .package",
    files:"Archivos", destination:"Destino", empty:"Elige uno o más archivos .Sims3Pack para previsualizar la conversión.",
    inspecting:"Leyendo Sims3Pack…", converting:"Convirtiendo…", ready:"Listo para convertir",
    set:n=>`Set · ${n} packages`, single:"Package único", nonPackage:"No es package",
    manifest:"Nombre del manifest", sims3packManifest:"Nombre localizado del Sims3Pack", casp:"Nombre del CASP",
    packaged:"Nombre del archivo interno", fallback:"Nombre alternativo", invalid:"Inválido",
    converted:n=>`${n} package(s) convertido(s)`, skipped:n=>`${n} omitido(s)`,
    noPackages:"No se encontraron payloads .package convertibles.", openDestination:"Abrir destino",
    warning:"Aviso", source:"Origen del nombre", output:"Salida", internal:"Archivo interno",
    combined:"Conversión conjunta", separateHint:"Desmarcado: conversión separada",
    destinationHint:"Destino sugerido", combinedUnsafe:"La conversión conjunta no está disponible para mundos, solares, Sims, conjuntos de la Store y otros contenidos protegidos.",
    manifest_disambiguated:"Nombre interno distinto del manifiesto",
    detectedType:"Tipo identificado", storeSet:"Conjunto de la Store",
  }
};

function storedLanguage() {
  try {
    const prefs = JSON.parse(localStorage.getItem(PREFS_KEY) || "{}");
    return ["en","pt","es"].includes(prefs.language) ? prefs.language : "en";
  } catch {
    return "en";
  }
}

const state = {
  language: storedLanguage(),
  paths: [],
  destination: "",
  inspections: [],
  combined: false,
  busy: false,
  lastResult: null,
};

const q = selector => document.querySelector(selector);
const tr = key => TEXT[state.language]?.[key] ?? TEXT.en[key] ?? key;

function sourceLabel(value) {
  return {
    manifest: tr("manifest"),
    sims3pack_manifest: tr("sims3packManifest"),
    casp: tr("casp"),
    packaged_file: tr("packaged"),
    fallback: tr("fallback"),
    invalid: tr("invalid"),
    non_package: tr("nonPackage"),
    manifest_disambiguated: tr("manifest_disambiguated"),
  }[value] || value || "—";
}

function setStatus(message, error = false) {
  const node = q("#sims3pack-converter-state");
  if (!node) return;
  node.textContent = message || "";
  node.className = "scan-state" + (message ? (error ? " error" : " success") : "");
}

function setStaticText() {
  q("#sims3pack-converter-title").textContent = tr("title");
  q("#sims3pack-converter-intro").textContent = tr("intro");
  q("#sims3pack-choose-files span").textContent = tr("chooseFiles");
  q("#sims3pack-choose-destination span").textContent = tr("chooseDestination");
  q("#sims3pack-convert span").textContent = tr("convert");
  q("#sims3pack-files-label").textContent = tr("files");
  q("#sims3pack-destination-label").textContent = tr("destination");
  q("#sims3pack-combined-label").textContent = tr("combined");
  const empty = q("#sims3pack-converter-empty");
  if (empty) empty.textContent = tr("empty");
  render();
}

function hasUnsafeCombinedItems() {
  const safe = new Set(["object", "cas", "caspart", "clothing", "hair", "accessory", "pattern", "build", "buy"]);
  return state.inspections.some(group => (group.items || []).some(item => item.convertible && (item.storeSet || !safe.has(String(item.contentType || "").toLowerCase().trim()))));
}

function updateControls() {
  q("#sims3pack-files-path").textContent = state.paths.length
    ? state.paths.map(path => path.split(/[\\/]/).pop()).join(" · ")
    : "—";
  q("#sims3pack-files-path").title = state.paths.join("\n");
  q("#sims3pack-destination-path").textContent = state.destination || "—";
  q("#sims3pack-destination-path").title = state.destination || "";

  const convertButton = q("#sims3pack-convert");
  const hasConvertible =
    state.inspections.some(group => (group.items || []).some(item => item.convertible));
  const canShowConvert = !!state.destination && hasConvertible;

  const unsafeCombined = hasUnsafeCombinedItems();
  const combinedControl = q("#sims3pack-combined");
  if (combinedControl) {
    combinedControl.disabled = unsafeCombined;
    if (unsafeCombined && state.combined) {
      state.combined = false;
      combinedControl.checked = false;
    }
    combinedControl.title = unsafeCombined ? tr("combinedUnsafe") : "";
  }
  convertButton.classList.toggle("hidden", !canShowConvert);
  convertButton.disabled = state.busy || (state.combined && unsafeCombined);
}

function renderInspection() {
  const root = q("#sims3pack-converter-results");
  root.innerHTML = "";

  if (!state.paths.length) {
    const empty = document.createElement("div");
    empty.id = "sims3pack-converter-empty";
    empty.className = "tools-empty";
    empty.textContent = tr("empty");
    root.appendChild(empty);
    return;
  }

  if (!state.inspections.length && state.busy) {
    const loading = document.createElement("div");
    loading.className = "tools-empty";
    loading.textContent = tr("inspecting");
    root.appendChild(loading);
    return;
  }

  let convertible = 0;
  for (const inspection of state.inspections) {
    const group = document.createElement("section");
    group.className = "sims3pack-group";

    const header = document.createElement("div");
    header.className = "sims3pack-group-header";
    const titleWrap = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = inspection.displayName || inspection.fileName;
    const source = document.createElement("small");
    source.textContent = inspection.fileName;
    titleWrap.append(title, source);
    const badge = document.createElement("span");
    badge.className = "status-badge";
    badge.textContent = inspection.set ? tr("set")(inspection.packageCount) : tr("single");
    header.append(titleWrap, badge);
    group.appendChild(header);

    const items = document.createElement("div");
    items.className = "sims3pack-items";

    for (const item of inspection.items || []) {
      if (item.convertible) convertible++;
      const row = document.createElement("article");
      row.className = "sims3pack-item" + (item.convertible ? "" : " invalid");

      const main = document.createElement("div");
      main.className = "sims3pack-item-main";
      const output = document.createElement("strong");
      output.textContent = item.convertible ? item.proposedFileName : item.packagedName;
      const details = document.createElement("span");
      const sizeMb = Number(item.size || 0) / (1024 * 1024);
      details.textContent = `${sourceLabel(item.nameSource)} · ${sizeMb >= 1 ? sizeMb.toFixed(2) + " MB" : Math.max(1, Math.round(Number(item.size || 0) / 1024)) + " KB"}`;
      main.append(output, details);
      if (item.convertible) {
        const type = document.createElement("span");
        type.textContent = `${tr("detectedType")}: ${item.storeSet ? tr("storeSet") : (item.contentType || "—")}`;
        main.appendChild(type);
        const category = document.createElement("span");
        category.textContent = `${tr("destinationHint")}: ${item.recommendedFolder || "Mods/Packages"}`;
        main.appendChild(category);
      }

      const technical = document.createElement("div");
      technical.className = "sims3pack-item-technical";
      const internal = document.createElement("code");
      internal.textContent = item.packagedName || "—";
      technical.appendChild(internal);

      row.append(main, technical);

      if (item.warning) {
        const warning = document.createElement("small");
        warning.className = "sims3pack-warning";
        warning.textContent = item.warning;
        row.appendChild(warning);
      }

      items.appendChild(row);
    }

    if (!(inspection.items || []).length) {
      const empty = document.createElement("div");
      empty.className = "tools-empty";
      empty.textContent = tr("noPackages");
      items.appendChild(empty);
    }

    group.appendChild(items);
    root.appendChild(group);
  }

  if (!convertible && state.inspections.length) {
    setStatus(tr("noPackages"), true);
  } else if (convertible && !state.busy && !state.lastResult) {
    setStatus(tr("ready"));
  }
}

function renderConversionResult() {
  if (!state.lastResult) return;
  const root = q("#sims3pack-converter-results");
  const summary = document.createElement("section");
  summary.className = "sims3pack-conversion-summary";

  const heading = document.createElement("div");
  heading.className = "sims3pack-conversion-summary-head";
  const text = document.createElement("strong");
  text.textContent = `${tr("converted")(state.lastResult.converted || 0)} · ${tr("skipped")(state.lastResult.skipped || 0)}`;
  heading.appendChild(text);

  if (state.destination) {
    const reveal = document.createElement("button");
    reveal.type = "button";
    reveal.className = "secondary-btn compact-btn";
    reveal.textContent = tr("openDestination");
    reveal.addEventListener("click", () => invoke("reveal_path", {path:state.destination}));
    heading.appendChild(reveal);
  }

  summary.appendChild(heading);

  for (const item of state.lastResult.items || []) {
    const line = document.createElement("div");
    line.className = "sims3pack-converted-line";
    const name = document.createElement("strong");
    name.textContent = item.outputFileName;
    const path = document.createElement("code");
    path.textContent = item.outputPath;
    line.append(name, path);
    summary.appendChild(line);
  }

  for (const error of state.lastResult.errors || []) {
    const line = document.createElement("div");
    line.className = "sims3pack-warning";
    line.textContent = error;
    summary.appendChild(line);
  }

  root.prepend(summary);
}

function render() {
  updateControls();
  renderInspection();
  renderConversionResult();
}

async function inspectSelected() {
  state.lastResult = null;
  if (!state.paths.length) {
    state.inspections = [];
    render();
    return;
  }

  state.busy = true;
  setStatus(tr("inspecting"));
  render();
  try {
    state.inspections = await invoke("inspect_sims3packs", {
      paths: state.paths,
      language: state.language,
    });
    setStatus(tr("ready"));
  } catch (error) {
    state.inspections = [];
    setStatus(String(error), true);
  } finally {
    state.busy = false;
    render();
  }
}

async function chooseFiles() {
  const selected = await open({
    multiple: true,
    directory: false,
    title: tr("chooseFiles"),
    filters: [{name:"The Sims 3 Pack", extensions:["sims3pack"]}],
  });
  if (!selected) return;
  state.paths = Array.isArray(selected) ? selected : [selected];
  await inspectSelected();
}

async function chooseDestination() {
  const selected = await open({
    multiple: false,
    directory: true,
    title: tr("chooseDestination"),
  });
  if (!selected || Array.isArray(selected)) return;
  state.destination = selected;
  render();
}

async function convert() {
  if (!state.paths.length || !state.destination || state.busy) return;
  if (state.combined && hasUnsafeCombinedItems()) {
    setStatus(tr("combinedUnsafe"), true);
    return;
  }
  state.busy = true;
  state.lastResult = null;
  setStatus(tr("converting"));
  render();
  try {
    state.lastResult = await invoke("convert_sims3packs", {
      paths: state.paths,
      destinationFolder: state.destination,
      language: state.language,
      combined: state.combined,
    });
    setStatus(tr("converted")(state.lastResult.converted || 0));
  } catch (error) {
    setStatus(String(error), true);
  } finally {
    state.busy = false;
    render();
  }
}

q("#sims3pack-choose-files")?.addEventListener("click", chooseFiles);
q("#sims3pack-choose-destination")?.addEventListener("click", chooseDestination);
q("#sims3pack-convert")?.addEventListener("click", convert);
q("#sims3pack-combined")?.addEventListener("change", event => {
  state.combined = Boolean(event.currentTarget.checked);
  state.lastResult = null;
  render();
});

window.addEventListener("s3cc-language-changed", event => {
  state.language = event.detail || "en";
  if (state.paths.length) {
    void inspectSelected();
  } else {
    setStaticText();
  }
});

setStaticText();
