import { FC } from "react";

export const FallbackView: FC<{name: string, message: string}> = ({name, message}) => {
  return (
    <>
      <p>{name}:</p>
      <p>{message}</p>
    </>
  );
};
