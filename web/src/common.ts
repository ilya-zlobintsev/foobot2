async function getJson(path: string) {
    const response = await fetch(path);
    if (response.ok) {
        return await response.json() 
    } else {
        return {};
    }
}

export { getJson }