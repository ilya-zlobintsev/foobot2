async function getJson(path: string) {
    const fullPath = BASE_URL + path;
    const response = await fetch(fullPath);
    if (response.ok) {
        return await response.json()
    } else {
        return {};
    }
}

export { getJson }