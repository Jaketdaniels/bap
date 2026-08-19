declare namespace Bap {
  type Unit<Name extends string> = number & {
    readonly __bapUnit: (name: Name) => Name;
  };

  const numeric: {
    readonly unit: <const Name extends string>(name: Name, value: number) => Unit<Name>;
    readonly unitAdd: <const Name extends string>(left: Unit<Name>, right: Unit<Name>) => Unit<Name>;
    readonly unitSub: <const Name extends string>(left: Unit<Name>, right: Unit<Name>) => Unit<Name>;
    readonly unitMul: (left: Unit<"dimensionless">, right: Unit<"dimensionless">) => Unit<"dimensionless">;
    readonly unitDiv: (left: Unit<"dimensionless">, right: Unit<"dimensionless">) => Unit<"dimensionless">;
  };
}
