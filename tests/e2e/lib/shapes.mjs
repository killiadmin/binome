// Forme (« shape ») des JSON échangés : c'est le contrat que le backend Rust doit
// reproduire. Les valeurs changent d'une partie à l'autre (tirage aléatoire,
// ids) ; la forme — clés, types, nullabilité, format des dates — ne doit pas.
//
// Une forme est une table « chemin → ensemble de types » :
//   players[].id          : int
//   card_draw             : null | object
//   created_at            : date:eloquent        (2026-11-11T10:00:00.000000Z)
//   played_at             : date:iso8601         (2026-11-11T10:00:00+00:00)

const ELOQUENT_DATE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/
const ISO8601_DATE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$/

/** Type d'une valeur scalaire, avec distinction des formats de date. */
export function scalarType(value) {
    if (value === null) return 'null'
    if (typeof value === 'boolean') return 'bool'
    // JSON ne distingue pas 2 de 2.0 : le front non plus, on ne le fait pas.
    if (typeof value === 'number') return Number.isInteger(value) ? 'int' : 'float'
    if (typeof value === 'string') {
        if (ELOQUENT_DATE.test(value)) return 'date:eloquent'
        if (ISO8601_DATE.test(value)) return 'date:iso8601'
        return 'string'
    }
    return typeof value
}

/**
 * Ajoute la forme de `value` dans `shape` (Map chemin → Set de types).
 * Les objets à clés numériques (maps id → valeur) sont repliés sur « {*} ».
 */
export function addShape(shape, value, path = '$') {
    const add = (p, t) => {
        if (!shape.has(p)) shape.set(p, new Set())
        shape.get(p).add(t)
    }

    if (Array.isArray(value)) {
        add(path, 'array')
        value.forEach((item) => addShape(shape, item, `${path}[]`))
        return shape
    }

    if (value !== null && typeof value === 'object') {
        const keys = Object.keys(value)
        if (keys.length > 0 && keys.every((k) => /^\d+$/.test(k))) {
            add(path, 'map')
            keys.forEach((k) => addShape(shape, value[k], `${path}{*}`))
            return shape
        }
        add(path, 'object')
        keys.forEach((k) => addShape(shape, value[k], `${path}.${k}`))
        return shape
    }

    // int et float se confondent en « number » si les deux apparaissent (moyennes)
    add(path, scalarType(value))
    return shape
}

/** Map → objet JSON trié, pour l'enregistrement sur disque. */
export function serializeShape(shape) {
    return Object.fromEntries(
        [...shape.keys()].sort().map((k) => [k, [...shape.get(k)].sort().join(' | ')]),
    )
}

/**
 * Compare deux catalogues { clé → forme sérialisée }.
 * Renvoie la liste des écarts lisibles. `int` ⊂ `float` côté nombres.
 */
export function compareCatalogs(reference, candidate) {
    const issues = []
    const norm = (t) => new Set(t.split(' | ').map((x) => (x === 'int' || x === 'float' ? 'number' : x)))

    for (const [key, refShape] of Object.entries(reference)) {
        const candShape = candidate[key]
        if (!candShape) {
            issues.push(`ABSENT       ${key}`)
            continue
        }
        for (const [path, refTypes] of Object.entries(refShape)) {
            if (!(path in candShape)) {
                issues.push(`CHAMP MANQUANT ${key}  ${path} (${refTypes})`)
                continue
            }
            const a = norm(refTypes)
            const b = norm(candShape[path])
            const extra = [...b].filter((t) => !a.has(t))
            if (extra.length) issues.push(`TYPE         ${key}  ${path}: référence « ${refTypes} », obtenu « ${candShape[path]} »`)
        }
        for (const path of Object.keys(candShape)) {
            if (!(path in refShape)) issues.push(`CHAMP EN TROP ${key}  ${path} (${candShape[path]})`)
        }
    }
    for (const key of Object.keys(candidate)) {
        if (!(key in reference)) issues.push(`NOUVEAU      ${key}`)
    }
    return issues
}
