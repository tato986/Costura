/* Catálogo de prendas, medidas y dibujo del patrón (SVG). */
(function () {
  const GARMENTS = {
    camisa: {
      label: "Camisa / blusa",
      icon: "i-shirt",
      styles: ["Clásico", "Mao", "Oversize", "Camisero", "Con pinzas"],
      fields: [
        { key: "pecho", label: "Pecho", min: 60, max: 160, def: 98 },
        { key: "cintura", label: "Cintura", min: 50, max: 150, def: 84 },
        { key: "cadera", label: "Cadera", min: 60, max: 160, def: 99 },
        { key: "hombros", label: "Ancho de hombros", min: 28, max: 60, def: 45 },
        { key: "largoManga", label: "Largo de manga", min: 15, max: 75, def: 63 },
        { key: "anchoManga", label: "Ancho de manga", min: 20, max: 55, def: 35 },
        { key: "largoTotal", label: "Largo total", min: 45, max: 110, def: 74 },
        { key: "cuello", label: "Contorno de cuello", min: 28, max: 55, def: 41 },
      ],
      options: [
        { key: "cuello", label: "Tipo de cuello", choices: ["Clásico", "Mao", "V", "Redondo"] },
        { key: "manga", label: "Manga", choices: ["Larga", "Corta"] },
      ],
    },
    pantalon: {
      label: "Pantalón",
      icon: "i-pants",
      styles: ["Recto", "Baggy", "Skinny", "Acampanado", "Cargo"],
      fields: [
        { key: "cintura", label: "Cintura", min: 50, max: 140, def: 84 },
        { key: "cadera", label: "Cadera", min: 60, max: 150, def: 104 },
        { key: "muslo", label: "Muslo", min: 30, max: 90, def: 60 },
        { key: "rodilla", label: "Rodilla", min: 22, max: 70, def: 46 },
        { key: "bota", label: "Bota (ruedo)", min: 18, max: 80, def: 44 },
        { key: "tiro", label: "Tiro", min: 18, max: 45, def: 30 },
        { key: "largoTotal", label: "Largo total", min: 70, max: 125, def: 106 },
      ],
      options: [
        { key: "cargo", label: "Bolsillos cargo", choices: ["No", "Sí"] },
      ],
      presets: {
        Recto: { cintura: 82, cadera: 100, muslo: 58, rodilla: 46, bota: 40, tiro: 28, largoTotal: 104 },
        Baggy: { cintura: 84, cadera: 108, muslo: 64, rodilla: 54, bota: 52, tiro: 32, largoTotal: 106 },
        Skinny: { cintura: 76, cadera: 94, muslo: 50, rodilla: 38, bota: 28, tiro: 26, largoTotal: 104 },
        Acampanado: { cintura: 78, cadera: 98, muslo: 54, rodilla: 42, bota: 60, tiro: 28, largoTotal: 108 },
        Cargo: { cintura: 86, cadera: 106, muslo: 62, rodilla: 50, bota: 46, tiro: 32, largoTotal: 106 },
      },
    },
    vestido: {
      label: "Vestido",
      icon: "i-dress",
      styles: ["Línea A", "Tubo", "Acampanado", "Maxi", "Camisero"],
      fields: [
        { key: "busto", label: "Busto", min: 60, max: 150, def: 90 },
        { key: "cintura", label: "Cintura", min: 50, max: 140, def: 72 },
        { key: "cadera", label: "Cadera", min: 60, max: 150, def: 96 },
        { key: "hombros", label: "Ancho de hombros", min: 28, max: 55, def: 38 },
        { key: "largoTalle", label: "Largo de talle", min: 30, max: 55, def: 39 },
        { key: "largoFalda", label: "Largo de falda", min: 35, max: 125, def: 70 },
      ],
      options: [
        { key: "escote", label: "Escote", choices: ["Redondo", "En V", "Corazón", "Barco"] },
        { key: "mangas", label: "Mangas", choices: ["Sin mangas", "Con mangas"] },
      ],
    },
    bolso: {
      label: "Bolso",
      icon: "i-bag",
      styles: ["Tote", "Bandolera", "Mochila", "Cartera"],
      fields: [
        { key: "ancho", label: "Ancho", min: 18, max: 65, def: 40 },
        { key: "alto", label: "Alto", min: 15, max: 50, def: 32 },
        { key: "profundidad", label: "Profundidad", min: 4, max: 25, def: 14 },
        { key: "largoCorrea", label: "Largo de correa/asas", min: 35, max: 140, def: 95 },
      ],
      options: [
        { key: "bolsillo", label: "Bolsillo externo", choices: ["No", "Sí"] },
        { key: "cierre", label: "Cierre", choices: ["No", "Sí"] },
      ],
    },
    otro: {
      label: "Otro / Personalizado",
      icon: "i-layers",
      styles: ["A medida", "Arreglo / ajuste", "Patrón libre", "Hogar / decoración", "Accesorio", "Otro"],
      fields: [
        { key: "largo", label: "Largo total", min: 5, max: 300, def: 100 },
        { key: "ancho", label: "Ancho", min: 5, max: 300, def: 80 },
      ],
      options: [
        { key: "acabado", label: "Acabado", choices: ["Estándar", "Reforzado", "Delicado", "Rústico"] },
      ],
    },
  };

  const clamp = (n, a, b) => Math.max(a, Math.min(b, n));
  const n = (v, fallback) => (typeof v === "number" && isFinite(v) ? v : fallback);

  function defaultValues(kind) {
    const spec = GARMENTS[kind];
    const out = {};
    spec.fields.forEach((f) => (out[f.key] = f.def));
    return out;
  }

  function defaultOptions(kind) {
    const out = {};
    GARMENTS[kind].options.forEach((o) => (out[o.key] = o.choices[0]));
    return out;
  }

  function measureSummary(kind, values) {
    const spec = GARMENTS[kind];
    if (!spec) return [];
    return spec.fields
      .filter((f) => typeof values[f.key] === "number")
      .map((f) => ({ key: f.key, label: f.label, value: values[f.key] }));
  }

  // ── piezas de dibujo ─────────────────────────────────────────────
  function svgWrap(inner, w, h) {
    return `<svg viewBox="0 0 ${w} ${h}" role="img" aria-label="Vista previa del patrón">${inner}</svg>`;
  }

  function hLabel(x1, x2, y, text) {
    const mid = (x1 + x2) / 2;
    return `<g stroke="currentColor" fill="none">
      <line x1="${x1}" y1="${y}" x2="${x2}" y2="${y}" stroke-width="0.9" stroke-dasharray="3 3" opacity=".38"/>
      <line x1="${x1}" y1="${y - 3}" x2="${x1}" y2="${y + 3}" stroke-width="0.9" opacity=".38"/>
      <line x1="${x2}" y1="${y - 3}" x2="${x2}" y2="${y + 3}" stroke-width="0.9" opacity=".38"/>
    </g>
    <text x="${mid}" y="${y - 4}" text-anchor="middle" font-size="8.5" fill="currentColor" opacity=".6" stroke="none">${text}</text>`;
  }

  function vLabel(x, y1, y2, text) {
    const mid = (y1 + y2) / 2;
    return `<g stroke="currentColor" fill="none">
      <line x1="${x}" y1="${y1}" x2="${x}" y2="${y2}" stroke-width="0.9" stroke-dasharray="3 3" opacity=".38"/>
      <line x1="${x - 3}" y1="${y1}" x2="${x + 3}" y2="${y1}" stroke-width="0.9" opacity=".38"/>
      <line x1="${x - 3}" y1="${y2}" x2="${x + 3}" y2="${y2}" stroke-width="0.9" opacity=".38"/>
    </g>
    <text x="${x + 5}" y="${mid + 3}" font-size="8.5" fill="currentColor" opacity=".6" stroke="none">${text}</text>`;
  }

  function buttons(cx, y1, y2, count) {
    let s = "";
    for (let i = 0; i < count; i++) {
      const y = y1 + ((y2 - y1) * i) / (count - 1);
      s += `<circle cx="${cx}" cy="${y}" r="1.8" fill="currentColor" opacity=".45"/>`;
    }
    return s;
  }

  // ── camisa ──────────────────────────────────────────────────────
  function shirt(v, o, showLabels) {
    const cx = 120;
    const chest = n(v.pecho, 98);
    const waist = n(v.cintura, 84);
    const hip = n(v.cadera, 99);
    const shoulder = n(v.hombros, 45);
    const sleeveLen = n(v.largoManga, 63);
    const sleeveW = n(v.anchoManga, 35);
    const length = n(v.largoTotal, 74);
    const neck = n(v.cuello, 41);

    const shoulderY = 62;
    const neckHalf = clamp(neck * 0.42, 13, 24);
    const shoulderHalf = clamp(shoulder * 1.15, 40, 66);
    const bodyTop = clamp(chest * 0.56, 46, 70);
    const waistHalf = clamp(waist * 0.5, 36, 62);
    const hipHalf = clamp(hip * 0.54, 42, 68);
    const armpitY = shoulderY + 34;
    const hemY = clamp(shoulderY + length * 2.15, 175, 316);
    const waistY = shoulderY + (hemY - shoulderY) * 0.56;

    const longSleeve = o.manga !== "Corta";
    const len = longSleeve ? clamp(sleeveLen * 0.95, 34, 78) : 22;
    const cuffHalf = clamp(sleeveW * 0.4, 10, 17);

    const body = [
      `M ${cx - neckHalf} ${shoulderY + 2}`,
      `C ${cx - neckHalf - 8} ${shoulderY - 2} ${cx - shoulderHalf + 10} ${shoulderY - 5} ${cx - shoulderHalf} ${shoulderY + 3}`,
      `C ${cx - shoulderHalf - 1} ${shoulderY + 16} ${cx - bodyTop - 3} ${armpitY - 18} ${cx - bodyTop} ${armpitY}`,
      `C ${cx - bodyTop - 4} ${armpitY + 22} ${cx - waistHalf} ${waistY - 26} ${cx - waistHalf} ${waistY}`,
      `C ${cx - waistHalf} ${waistY + 28} ${cx - hipHalf} ${hemY - 24} ${cx - hipHalf} ${hemY}`,
      `Q ${cx} ${hemY + 8} ${cx + hipHalf} ${hemY}`,
      `C ${cx + hipHalf} ${hemY - 24} ${cx + waistHalf} ${waistY + 28} ${cx + waistHalf} ${waistY}`,
      `C ${cx + waistHalf} ${waistY - 26} ${cx + bodyTop + 4} ${armpitY + 22} ${cx + bodyTop} ${armpitY}`,
      `C ${cx + bodyTop + 3} ${armpitY - 18} ${cx + shoulderHalf + 1} ${shoulderY + 16} ${cx + shoulderHalf} ${shoulderY + 3}`,
      `C ${cx + shoulderHalf - 10} ${shoulderY - 5} ${cx + neckHalf + 8} ${shoulderY - 2} ${cx + neckHalf} ${shoulderY + 2}`,
    ].join(" ");

    const sleeveSide = (sign) => {
      const sx = cx + sign * shoulderHalf;
      return [
        `M ${sx} ${shoulderY + 3}`,
        `L ${sx + sign * (cuffHalf + 6)} ${shoulderY + len * 0.82}`,
        `L ${sx + sign * (cuffHalf - 2)} ${shoulderY + len * 0.98}`,
        `L ${cx + sign * (bodyTop - 1)} ${armpitY + 7}`,
        "Z",
      ].join(" ");
    };

    const collarType = o.cuello || "Clásico";
    let collar = "";
    if (collarType === "V") {
      collar = `<path d="M ${cx - neckHalf} ${shoulderY + 2} L ${cx} ${shoulderY + 44} L ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;
    } else if (collarType === "Mao") {
      collar = `<path d="M ${cx - neckHalf - 2} ${shoulderY + 2} Q ${cx} ${shoulderY - 5} ${cx + neckHalf + 2} ${shoulderY + 2} L ${cx + neckHalf + 1} ${shoulderY + 13} Q ${cx} ${shoulderY + 8} ${cx - neckHalf - 1} ${shoulderY + 13} Z" fill="currentColor" fill-opacity=".12" stroke="currentColor" stroke-width="1.4"/>`;
    } else if (collarType === "Redondo") {
      collar = `<path d="M ${cx - neckHalf} ${shoulderY + 2} Q ${cx} ${shoulderY + 26} ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;
    } else {
      collar = `<path d="M ${cx - neckHalf} ${shoulderY + 2} L ${cx - 5} ${shoulderY + 30} L ${cx} ${shoulderY + 24} L ${cx + 5} ${shoulderY + 30} L ${cx + neckHalf} ${shoulderY + 2} Z" fill="currentColor" fill-opacity=".14" stroke="currentColor" stroke-width="1.4"/>`;
    }

    const cuffLines = longSleeve
      ? `<path d="M ${cx - shoulderHalf - 4 - cuffHalf} ${shoulderY + len * 0.82} L ${cx - shoulderHalf - cuffHalf + 2} ${shoulderY + len * 0.98}" stroke="currentColor" stroke-width="2" fill="none" opacity=".7"/>
         <path d="M ${cx + shoulderHalf + cuffHalf - 2} ${shoulderY + len * 0.98} L ${cx + shoulderHalf + 4 + cuffHalf} ${shoulderY + len * 0.82}" stroke="currentColor" stroke-width="2" fill="none" opacity=".7"/>`
      : "";

    const labels = showLabels
      ? hLabel(cx - bodyTop, cx + bodyTop, armpitY + 34, "pecho") +
        vLabel(cx + hipHalf + 12, shoulderY, hemY, "largo")
      : "";

    return svgWrap(
      `<g fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
        <path d="${body}"/>
        <path d="${sleeveSide(-1)}"/>
        <path d="${sleeveSide(1)}"/>
      </g>
      ${collar}${cuffLines}
      <path d="M ${cx} ${shoulderY + 20} L ${cx} ${hemY - 2}" stroke="currentColor" stroke-width="1" stroke-dasharray="2 3" opacity=".55"/>
      ${buttons(cx, shoulderY + 34, hemY - 10, 5)}
      <path d="M ${cx - waistHalf + 2} ${waistY} Q ${cx} ${waistY + 4} ${cx + waistHalf - 2} ${waistY}" stroke="currentColor" stroke-width="1" fill="none" stroke-dasharray="3 3" opacity=".5"/>
      ${labels}`,
      240,
      340
    );
  }

  // ── pantalón ────────────────────────────────────────────────────
  function pants(v, o, showLabels) {
    const cx = 120;
    const waist = n(v.cintura, 84);
    const hip = n(v.cadera, 104);
    const thigh = n(v.muslo, 60);
    const knee = n(v.rodilla, 46);
    const hem = n(v.bota, 44);
    const rise = n(v.tiro, 30);
    const length = n(v.largoTotal, 106);

    const waistY = 58;
    const waistHalf = clamp(waist * 0.6, 34, 60);
    const hipY = waistY + clamp(rise * 0.9, 24, 40);
    const hipHalf = clamp(hip * 0.62, 42, 72);
    const crotchY = waistY + clamp(rise * 3.2, 60, 96);
    const hemY = clamp(crotchY + (length - rise) * 1.75, 210, 322);
    const kneeY = crotchY + (hemY - crotchY) * 0.52;

    const hemOuter = clamp(hipHalf + hem * 0.32 - 4, hipHalf - 4, 104);
    const hemInner = 8;
    const kneeOuter = hipHalf + (hemOuter - hipHalf) * 0.55;
    const kneeInner = clamp(8 + (thigh - knee) * 0.15, 8, 20);
    const innerCrotch = clamp(thigh * 0.16, 4, 12);

    const d = [
      `M ${cx - waistHalf} ${waistY}`,
      `L ${cx - hipHalf} ${hipY}`,
      `C ${cx - hipHalf - 2} ${hipY + 22} ${cx - kneeOuter} ${kneeY - 26} ${cx - kneeOuter} ${kneeY}`,
      `L ${cx - hemOuter} ${hemY - 8}`,
      `Q ${cx - hemOuter + 2} ${hemY} ${cx - hemOuter + 7} ${hemY}`,
      `L ${cx - hemInner - 8} ${hemY}`,
      `Q ${cx - hemInner - 1} ${hemY - 4} ${cx - hemInner} ${hemY - 10}`,
      `L ${cx - kneeInner - 2} ${kneeY + 6}`,
      `L ${cx - innerCrotch} ${crotchY}`,
      `Q ${cx} ${crotchY + 6} ${cx + innerCrotch} ${crotchY}`,
      `L ${cx + kneeInner + 2} ${kneeY + 6}`,
      `L ${cx + hemInner} ${hemY - 10}`,
      `Q ${cx + hemInner + 1} ${hemY - 4} ${cx + hemInner + 8} ${hemY}`,
      `L ${cx + hemOuter - 7} ${hemY}`,
      `Q ${cx + hemOuter - 2} ${hemY} ${cx + hemOuter} ${hemY - 8}`,
      `L ${cx + kneeOuter} ${kneeY}`,
      `C ${cx + kneeOuter} ${kneeY - 26} ${cx + hipHalf + 2} ${hipY + 22} ${cx + hipHalf} ${hipY}`,
      `L ${cx + waistHalf} ${waistY}`,
      "Z",
    ].join(" ");

    const cargo =
      o.cargo === "Sí"
        ? `<rect x="${cx - hipHalf - 2}" y="${kneeY - 34}" width="20" height="22" rx="2" fill="currentColor" fill-opacity=".06" stroke="currentColor" stroke-width="1.1" stroke-dasharray="3 2"/>
           <rect x="${cx + hipHalf - 18}" y="${kneeY - 34}" width="20" height="22" rx="2" fill="currentColor" fill-opacity=".06" stroke="currentColor" stroke-width="1.1" stroke-dasharray="3 2"/>`
        : "";

    const labels = showLabels
      ? hLabel(cx - waistHalf, cx + waistHalf, waistY - 9, "cintura") +
        vLabel(cx + hipHalf + 12, hipY, hemY, "largo")
      : "";

    return svgWrap(
      `<g fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="${d}"/></g>
       <path d="M ${cx - waistHalf} ${waistY + 9} L ${cx + waistHalf} ${waistY + 9}" stroke="currentColor" stroke-width="1.2" opacity=".55"/>
       <path d="M ${cx - waistHalf + 6} ${waistY + 4} L ${cx - waistHalf + 6} ${waistY + 9} M ${cx + waistHalf - 6} ${waistY + 4} L ${cx + waistHalf - 6} ${waistY + 9}" stroke="currentColor" stroke-width="1" opacity=".45"/>
       <path d="M ${cx} ${waistY + 9} L ${cx} ${crotchY - 6}" stroke="currentColor" stroke-width="1" stroke-dasharray="2 3" opacity=".5"/>
       <path d="M ${cx - hipHalf + 3} ${hipY + 2} Q ${cx - hipHalf + 14} ${hipY + 12} ${cx - hipHalf + 3} ${hipY + 22}" stroke="currentColor" stroke-width="1" fill="none" opacity=".45"/>
       <path d="M ${cx + hipHalf - 3} ${hipY + 2} Q ${cx + hipHalf - 14} ${hipY + 12} ${cx + hipHalf - 3} ${hipY + 22}" stroke="currentColor" stroke-width="1" fill="none" opacity=".45"/>
       ${cargo}${labels}`,
      240,
      340
    );
  }

  // ── vestido ─────────────────────────────────────────────────────
  function dress(v, o, showLabels) {
    const cx = 120;
    const bust = n(v.busto, 90);
    const waist = n(v.cintura, 72);
    const hip = n(v.cadera, 96);
    const shoulder = n(v.hombros, 38);
    const bodice = n(v.largoTalle, 39);
    const skirt = n(v.largoFalda, 70);
    const style = o.estilo || "Línea A";

    const shoulderY = 60;
    const shoulderHalf = clamp(shoulder * 1.05, 30, 52);
    const neckHalf = 15;
    const bustY = shoulderY + 34;
    const bustHalf = clamp(bust * 0.52, 38, 60);
    const waistY = clamp(shoulderY + bodice * 2.05, 130, 190);
    const waistHalf = clamp(waist * 0.44, 26, 48);
    const hipY = waistY + 24;
    const hipHalf = clamp(hip * 0.5, 34, 58);
    const flare = style === "Tubo" ? -8 : style === "Acampanado" ? 46 : style === "Maxi" ? 34 : 30;
    const hemHalf = clamp(hipHalf + flare, 34, 104);
    const hemY = clamp(waistY + skirt * 1.45, 210, 322);

    const d = [
      `M ${cx - neckHalf} ${shoulderY + 2}`,
      `L ${cx - shoulderHalf} ${shoulderY + 5}`,
      `L ${cx - bustHalf} ${bustY}`,
      `C ${cx - bustHalf} ${bustY + 14} ${cx - waistHalf - 2} ${waistY - 24} ${cx - waistHalf} ${waistY}`,
      `C ${cx - waistHalf} ${waistY + 14} ${cx - hipHalf} ${hipY + 6} ${cx - hipHalf} ${hipY}`,
      `L ${cx - hemHalf} ${hemY - 6}`,
      `Q ${cx} ${hemY + 6} ${cx + hemHalf} ${hemY - 6}`,
      `L ${cx + hipHalf} ${hipY}`,
      `C ${cx + hipHalf} ${hipY + 6} ${cx + waistHalf} ${waistY + 14} ${cx + waistHalf} ${waistY}`,
      `C ${cx + waistHalf + 2} ${waistY - 24} ${cx + bustHalf} ${bustY + 14} ${cx + bustHalf} ${bustY}`,
      `L ${cx + shoulderHalf} ${shoulderY + 5}`,
      `L ${cx + neckHalf} ${shoulderY + 2}`,
    ].join(" ");

    const escote = o.escote || "Redondo";
    let neck = "";
    if (escote === "En V") neck = `<path d="M ${cx - neckHalf} ${shoulderY + 2} L ${cx} ${shoulderY + 40} L ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;
    else if (escote === "Corazón") neck = `<path d="M ${cx - neckHalf} ${shoulderY + 2} Q ${cx - 8} ${shoulderY + 26} ${cx} ${shoulderY + 30} Q ${cx + 8} ${shoulderY + 26} ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;
    else if (escote === "Barco") neck = `<path d="M ${cx - neckHalf} ${shoulderY + 2} Q ${cx} ${shoulderY + 12} ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;
    else neck = `<path d="M ${cx - neckHalf} ${shoulderY + 2} Q ${cx} ${shoulderY + 22} ${cx + neckHalf} ${shoulderY + 2}" fill="none" stroke="currentColor" stroke-width="1.6"/>`;

    const sleeves =
      o.mangas === "Con mangas"
        ? `<path d="M ${cx - shoulderHalf} ${shoulderY + 5} L ${cx - shoulderHalf - 12} ${bustY + 4} L ${cx - bustHalf} ${bustY + 2}" fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.5"/>
           <path d="M ${cx + shoulderHalf} ${shoulderY + 5} L ${cx + shoulderHalf + 12} ${bustY + 4} L ${cx + bustHalf} ${bustY + 2}" fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.5"/>`
        : "";

    const waistSeam = `<path d="M ${cx - waistHalf} ${waistY} Q ${cx} ${waistY + 6} ${cx + waistHalf} ${waistY}" stroke="currentColor" stroke-width="1.1" fill="none" opacity=".6"/>`;

    const labels = showLabels
      ? hLabel(cx - bustHalf, cx + bustHalf, bustY + 14, "busto") +
        vLabel(Math.min(226, cx + hemHalf + 10), waistY, hemY, "falda")
      : "";

    return svgWrap(
      `<g fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="${d}"/></g>
       ${neck}${sleeves}${waistSeam}${labels}`,
      240,
      340
    );
  }

  // ── bolso ───────────────────────────────────────────────────────
  function bag(v, o, showLabels) {
    const cx = 120;
    const w = clamp(n(v.ancho, 40) * 3.4, 80, 200);
    const h = clamp(n(v.alto, 32) * 3.4, 70, 175);
    const depth = clamp(n(v.profundidad, 14) * 2.0, 12, 44);
    const strap = clamp(n(v.largoCorrea, 95) * 0.6, 42, 100);
    const top = 182 - h / 2;
    const left = cx - w / 2;
    const right = cx + w / 2;
    const bottom = top + h;
    const dx = depth * 0.6;
    const dy = depth * 0.55;
    const type = o.estilo || o.tipo || "Tote";

    const straps = (() => {
      if (type === "Mochila") {
        const d = [
          `M ${left + w * 0.3} ${top + 4}`,
          `C ${left + w * 0.22} ${top - strap * 0.8} ${left + w * 0.42} ${top - strap} ${left + w * 0.5} ${top - strap}`,
          `M ${right - w * 0.3} ${top + 4}`,
          `C ${right - w * 0.22} ${top - strap * 0.8} ${right - w * 0.42} ${top - strap} ${right - w * 0.5} ${top - strap}`,
        ].join(" ");
        return `<path d="${d}" fill="none" stroke="currentColor" stroke-width="4.5" stroke-linecap="round" opacity=".6"/>`;
      }
      if (type === "Bandolera") {
        const d = `M ${left + 10} ${top + 2} C ${left - 6} ${top - strap} ${right + 6} ${top - strap} ${right - 10} ${top + 2}`;
        return `<path d="${d}" fill="none" stroke="currentColor" stroke-width="3.6" stroke-linecap="round" opacity=".62"/>`;
      }
      if (type === "Cartera") {
        const d = `M ${left + w * 0.34} ${top + 2} C ${left + w * 0.34} ${top - strap * 0.55} ${right - w * 0.34} ${top - strap * 0.55} ${right - w * 0.34} ${top + 2}`;
        return `<path d="${d}" fill="none" stroke="currentColor" stroke-width="3.4" stroke-linecap="round" opacity=".62"/>`;
      }
      const d = [
        `M ${left + w * 0.24} ${top + 2}`,
        `C ${left + w * 0.24} ${top - strap * 0.7} ${left + w * 0.4} ${top - strap * 0.7} ${left + w * 0.4} ${top + 2}`,
        `M ${right - w * 0.4} ${top + 2}`,
        `C ${right - w * 0.4} ${top - strap * 0.7} ${right - w * 0.24} ${top - strap * 0.7} ${right - w * 0.24} ${top + 2}`,
      ].join(" ");
      return `<path d="${d}" fill="none" stroke="currentColor" stroke-width="3.6" stroke-linecap="round" opacity=".62"/>`;
    })();

    const pocket =
      o.bolsillo === "Sí"
        ? `<rect x="${left + 10}" y="${bottom - h * 0.42}" width="${w * 0.38}" height="${h * 0.28}" rx="3" fill="currentColor" fill-opacity=".05" stroke="currentColor" stroke-width="1.1" stroke-dasharray="3 2"/>`
        : "";

    const zipper =
      o.cierre === "Sí"
        ? `<path d="M ${left + 7} ${top + 13} L ${right - 7} ${top + 13}" stroke="currentColor" stroke-width="1.4" stroke-dasharray="4 2" opacity=".7"/>
           <circle cx="${right - 9}" cy="${top + 13}" r="2" fill="currentColor" opacity=".55"/>`
        : "";

    const labels = showLabels
      ? vLabel(left - 12, top, bottom, "alto") +
        hLabel(left, right, bottom + 15, "ancho")
      : "";

    return svgWrap(
      `<g>
        ${straps}
        <rect x="${left + dx}" y="${top - dy}" width="${w}" height="${h}" rx="6" fill="currentColor" fill-opacity=".04" stroke="currentColor" stroke-width="1.1" opacity=".45"/>
        <path d="M ${left} ${top} L ${left + dx} ${top - dy} M ${right} ${top} L ${right + dx} ${top - dy} M ${right} ${bottom} L ${right + dx} ${bottom - dy}"
              stroke="currentColor" stroke-width=".9" opacity=".35" fill="none"/>
        <rect x="${left}" y="${top}" width="${w}" height="${h}" rx="6" fill="currentColor" fill-opacity=".09" stroke="currentColor" stroke-width="1.8"/>
        <path d="M ${left + 2} ${top + 15} L ${right - 2} ${top + 15}" stroke="currentColor" stroke-width="1" opacity=".4"/>
        ${zipper}${pocket}
        <path d="M ${left + w * 0.5} ${top} L ${left + w * 0.5} ${bottom}" stroke="currentColor" stroke-width="1" stroke-dasharray="2 3" opacity=".3"/>
      </g>
      ${labels}`,
      240,
      340
    );
  }

  function otherPattern(v, o, show) {
    const w = clamp(n(v.ancho, 80), 30, 200);
    const h = clamp(n(v.largo, 100), 40, 240);
    const left = 120 - w * 0.45;
    const top = 170 - h * 0.45;
    const right = left + w * 0.9;
    const bottom = top + h * 0.9;

    const labels = show
      ? vLabel(left - 14, top, bottom, "largo") +
        hLabel(left, right, bottom + 15, "ancho")
      : "";

    return svgWrap(
      `<g>
        <rect x="${left}" y="${top}" width="${right - left}" height="${bottom - top}" rx="8" fill="currentColor" fill-opacity=".06" stroke="currentColor" stroke-width="1.6" stroke-dasharray="4 3"/>
        <line x1="${left + 15}" y1="${top + 20}" x2="${right - 15}" y2="${top + 20}" stroke="currentColor" stroke-width="0.9" opacity=".4"/>
        <line x1="${left + 15}" y1="${bottom - 20}" x2="${right - 15}" y2="${bottom - 20}" stroke="currentColor" stroke-width="0.9" opacity=".4"/>
        <circle cx="120" cy="170" r="28" fill="currentColor" fill-opacity=".08" stroke="currentColor" stroke-width="1.1" opacity=".5"/>
        <path d="M 120 152 L 120 188 M 102 170 L 138 170" stroke="currentColor" stroke-width="1" opacity=".35"/>
        <text x="120" y="174" text-anchor="middle" font-size="8" fill="currentColor" opacity=".7" font-weight="600" stroke="none">A MEDIDA</text>
      </g>
      ${labels}`,
      240,
      340
    );
  }

  function garmentSvg(kind, values, options, opts) {
    const show = !opts || opts.labels !== false;
    const v = values || {};
    const o = options || {};
    let inner;
    if (kind === "pantalon") inner = pants(v, o, show);
    else if (kind === "vestido") inner = dress(v, o, show);
    else if (kind === "bolso") inner = bag(v, o, show);
    else if (kind === "otro") inner = otherPattern(v, o, show);
    else inner = shirt(v, o, show);
    return inner;
  }

  window.Costura = window.Costura || {};
  window.Costura.garments = {
    GARMENTS,
    defaultValues,
    defaultOptions,
    measureSummary,
    render: garmentSvg,
    label: (kind) => (GARMENTS[kind] ? GARMENTS[kind].label : kind),
    styles: (kind) => (GARMENTS[kind] ? GARMENTS[kind].styles : []),
    presets: (kind) => (GARMENTS[kind] && GARMENTS[kind].presets) || null,
  };
})();
