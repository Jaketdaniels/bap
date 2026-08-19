const greeting: string = "hello from bap TypeScript";
const values: number[] = [1, 2];

console.log(greeting);
console.log("sum:", values.reduce((sum, value) => sum + value, 0));
