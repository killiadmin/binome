// Identité de repli pour le chat du hall : un visiteur qui n'a ni créé ni
// rejoint de partie n'a pas de Player en base, on lui tire donc un identifiant
// stable, conservé dans le localStorage du navigateur. Le backend en dérive le
// nom affiché (`Anonyme#1234`).
const STORAGE_KEY = 'chatAnonId'

export function getAnonId() {
    let id = localStorage.getItem(STORAGE_KEY)

    if (!id || !/^[A-Za-z0-9]{1,12}$/.test(id)) {
        id = String(Math.floor(1000 + Math.random() * 9000))
        localStorage.setItem(STORAGE_KEY, id)
    }

    return id
}

export function anonName(id = getAnonId()) {
    return `Anonyme#${id}`
}
