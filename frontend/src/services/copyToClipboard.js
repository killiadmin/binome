// Copie un texte dans le presse-papier, avec repli execCommand pour les
// contextes non sécurisés (http:// sur IP LAN) où navigator.clipboard est absent.
// Renvoie true si la copie a réussi.
export async function copyToClipboard(text) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch (e) {
    /* on tente le repli ci-dessous */
  }
  try {
    const el = document.createElement('textarea')
    el.value = text
    el.setAttribute('readonly', '')
    el.style.position = 'absolute'
    el.style.left = '-9999px'
    document.body.appendChild(el)
    el.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(el)
    return ok
  } catch (e) {
    return false
  }
}
