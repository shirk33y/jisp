// export const new_ = (value?: any) => `${value}`;
// export const is = (value?: any) => typeof value === "string";
// export const cat = (...strs: string[]) => strs.join("");
// export const len = (s: string) => s.length;
// export const upper = (s: string) => s.toUpperCase();
// export const lower = (s: string) => s.toLowerCase();

export const str = {
  new: (value?: any) => `${value}`,
  is: (value?: any) => typeof value === "string",
  cat: (...strs: string[]) => strs.join(""),
  len: (s: string) => s.length,
  upper: (s: string) => s.toUpperCase(),
  lower: (s: string) => s.toLowerCase(),
};

export const num = {
  new: (value?: any) => parseFloat(value),
  is: (value?: any) => typeof value === "number",
};
