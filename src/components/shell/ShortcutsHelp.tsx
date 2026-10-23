import Modal from "../Modal";
import { Kbd } from "../ui";
import { t } from "../../i18n";

const S = t.shell.shortcuts;

export default function ShortcutsHelp({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} title={S.title} onClose={onClose} size="sm">
      <table className="data-table">
        <tbody>
          {S.items.map(([keys, what]) => (
            <tr key={keys}>
              <td className="whitespace-nowrap">
                {keys.split(" или ").map((k, i) => (
                  <span key={k}>
                    {i > 0 && " / "}
                    <Kbd>{k}</Kbd>
                  </span>
                ))}
              </td>
              <td>{what}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-3 text-xs text-zinc-500">{S.global}</p>
    </Modal>
  );
}
